use anyhow::{Context, Result};
use wgpu::*;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use wgpu::util::DeviceExt;
use std::sync::Arc;

/// WGPU 视频渲染器
/// 使用同步渲染模式，将解码后的 BGRA 像素渲染到窗口
pub struct WgpuRenderer {
    window: Option<Arc<Window>>,
    surface: Option<Surface<'static>>,
    device: Option<Device>,
    queue: Option<Queue>,
    config: Option<SurfaceConfiguration>,
    render_pipeline: Option<RenderPipeline>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
    texture: Option<Texture>,
    texture_view: Option<TextureView>,
    sampler: Option<Sampler>,
    bind_group: Option<BindGroup>,
    width: u32,
    height: u32,
}

/// 顶点结构：位置 + 纹理坐标
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coord: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [VertexAttribute; 2] = vertex_attr_array![
        0 => Float32x2, // 位置
        1 => Float32x2, // 纹理坐标
    ];

    const DESC: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: &Self::ATTRIBS,
    };
}

/// 全屏四边形顶点（位置 + 纹理坐标）
const VERTICES: &[Vertex] = &[
    // 左上
    Vertex {
        position: [-1.0, -1.0],
        tex_coord: [0.0, 0.0],
    },
    // 右上
    Vertex {
        position: [1.0, -1.0],
        tex_coord: [1.0, 0.0],
    },
    // 左下
    Vertex {
        position: [-1.0, 1.0],
        tex_coord: [0.0, 1.0],
    },
    // 右下
    Vertex {
        position: [1.0, 1.0],
        tex_coord: [1.0, 1.0],
    },
];

/// 索引（两个三角形组成四边形）
const INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

impl WgpuRenderer {
    /// 创建新的渲染器实例
    pub fn new() -> Self {
        Self {
            window: None,
            surface: None,
            device: None,
            queue: None,
            config: None,
            render_pipeline: None,
            vertex_buffer: None,
            index_buffer: None,
            texture: None,
            texture_view: None,
            sampler: None,
            bind_group: None,
            width: 0,
            height: 0,
        }
    }

    /// 获取窗口引用
    pub fn window(&self) -> Option<&Window> {
        self.window.as_deref()
    }

    /// 获取窗口尺寸
    pub fn size(&self) -> PhysicalSize<u32> {
        self.window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or(PhysicalSize::new(self.width, self.height))
    }

    /// 更新视频纹理
    /// 
    /// # 参数
    /// * `data` - BGRA 像素数据（每像素 4 字节）
    /// * `width` - 视频宽度
    /// * `height` - 视频高度
    /// 
    /// # 说明
    /// Phase 1a 假设数据已经是解码后的 BGRA 像素。
    /// 后续集成 VP9 解码时，在此处添加解码逻辑。
    pub fn update_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<()> {
        let queue = self.queue.as_ref().context("Queue not initialized")?;
        
        // 验证数据大小
        let expected_size = (width * height * 4) as usize;
        if data.len() < expected_size {
            anyhow::bail!(
                "Invalid texture data: expected {} bytes, got {}",
                expected_size,
                data.len()
            );
        }

        // 如果纹理不存在或尺寸变化，重新创建
        if self.texture.is_none() 
            || self.width != width 
            || self.height != height 
            || self.bind_group.is_none()
        {
            self.width = width;
            self.height = height;

            // 创建纹理
            self.texture = Some(self.device.as_ref().unwrap().create_texture(&TextureDescriptor {
                label: Some("video_texture"),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm, // BGRA 数据使用 Rgba8Unorm
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            }));

            // 创建纹理视图
            self.texture_view = Some(self.texture.as_ref().unwrap().create_view(&TextureViewDescriptor {
                label: Some("video_texture_view"),
                format: Some(TextureFormat::Rgba8Unorm),
                dimension: Some(TextureViewDimension::D2),
                aspect: TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            }));

            // 创建采样器
            self.sampler = Some(self.device.as_ref().unwrap().create_sampler(&SamplerDescriptor {
                label: Some("texture_sampler"),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Nearest,
                ..Default::default()
            }));

            // 创建 bind group
            self.bind_group = Some(self.device.as_ref().unwrap().create_bind_group(&BindGroupDescriptor {
                label: Some("texture_bind_group"),
                layout: &self.render_pipeline.as_ref().unwrap().get_bind_group_layout(0),
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(self.texture_view.as_ref().unwrap()),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(self.sampler.as_ref().unwrap()),
                    },
                ],
            }));
        }

        // 上传纹理数据
        queue.write_texture(
            ImageCopyTexture {
                texture: self.texture.as_ref().unwrap(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// 渲染一帧
    pub fn render(&self) -> Result<()> {
        let surface = self.surface.as_ref().context("Surface not initialized")?;
        let device = self.device.as_ref().context("Device not initialized")?;
        let queue = self.queue.as_ref().context("Queue not initialized")?;
        let render_pipeline = self.render_pipeline.as_ref().context("Pipeline not initialized")?;
        let bind_group = self.bind_group.as_ref().context("Bind group not initialized")?;

        // 获取下一个帧缓冲
        let frame = surface
            .get_current_texture()
            .context("Failed to acquire next swap chain texture")?;

        // 创建命令编码器
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        // 创建纹理视图（用于渲染通道）
        let frame_view = frame.texture.create_view(&TextureViewDescriptor::default());

        // 创建渲染通道
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &frame_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 设置渲染管线
            render_pass.set_pipeline(render_pipeline);

            // 设置顶点缓冲
            if let Some(vertex_buffer) = &self.vertex_buffer {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            }

            // 设置索引缓冲
            if let Some(index_buffer) = &self.index_buffer {
                render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
            }

            // 设置 bind group
            render_pass.set_bind_group(0, bind_group, &[]);

            // 绘制
            render_pass.draw_indexed(0..(INDICES.len() as u32), 0, 0..1);
        } // render_pass dropped here

        // 提交命令
        queue.submit(Some(encoder.finish()));

        // 呈现
        frame.present();

        Ok(())
    }

    /// 重新配置渲染器（窗口尺寸变化时调用）
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) -> Result<()> {
        if new_size.width == 0 || new_size.height == 0 {
            return Ok(());
        }

        let surface = self.surface.as_ref().context("Surface not initialized")?;
        let device = self.device.as_ref().context("Device not initialized")?;

        self.width = new_size.width;
        self.height = new_size.height;

        let mut config = self.config.take().context("Config not initialized")?;
        config.width = new_size.width;
        config.height = new_size.height;
        surface.configure(device, &config);

        self.config = Some(config);

        Ok(())
    }

    /// 初始化 WGPU（同步版本）
    fn init_wgpu(&mut self) {
        let Some(window) = self.window.as_ref() else {
            tracing::error!("Window not created");
            return;
        };

        // 创建实例
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        // 创建表面 - 使用 transmute 将生命周期转为 'static
        // 这是安全的，因为 window 的生命周期由 self.window 管理，
        // 在 WgpuRenderer 被 drop 之前 window 不会失效
        let surface_raw = instance.create_surface(window).unwrap();
        let surface: Surface<'static> = unsafe {
            std::mem::transmute(surface_raw)
        };

        // 请求适配器
        let adapter = futures::executor::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).expect("Failed to find suitable adapter");

        // 请求设备和队列
        let (device, queue) = futures::executor::block_on(adapter.request_device(
            &DeviceDescriptor {
                required_features: Features::empty(),
                required_limits: Limits::default(),
                label: Some("wgpu_device"),
            },
            None,
        )).expect("Failed to request device");

        // 获取表面能力
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // 配置表面
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // 创建渲染管线
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render_pipeline_layout"),
            bind_group_layouts: &[&device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::DESC],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // 创建顶点缓冲
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: BufferUsages::VERTEX,
        });

        // 创建索引缓冲
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: BufferUsages::INDEX,
        });

        // 保存状态
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.config = Some(config);
        self.render_pipeline = Some(render_pipeline);
        self.vertex_buffer = Some(vertex_buffer);
        self.index_buffer = Some(index_buffer);
    }

    /// 运行渲染器主循环
    pub fn run(mut self) -> Result<()> {
        let event_loop = EventLoop::new().context("Failed to create event loop")?;
        event_loop.set_control_flow(ControlFlow::Poll);

        // 运行事件循环
        event_loop.run_app(&mut self)?;

        Ok(())
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// ApplicationHandler 实现，用于 winit 事件循环
impl ApplicationHandler for WgpuRenderer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 创建窗口
        let window_attributes = Window::default_attributes()
            .with_title("RDP Remote - Video Renderer")
            .with_inner_size(PhysicalSize::new(800, 600))
            .with_resizable(true);

        let window = event_loop.create_window(window_attributes).unwrap();
        self.window = Some(Arc::new(window));

        // 初始化 WGPU
        self.init_wgpu();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                let _ = self.resize(physical_size);
            }
            WindowEvent::RedrawRequested => {
                // 渲染一帧
                let _ = self.render();
            }
            _ => {}
        }
    }
}
