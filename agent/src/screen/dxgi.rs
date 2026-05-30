use anyhow::{Context, Result};
use std::time::Instant;
use super::{CapturedFrame, ScreenCapture};
use windows::{
    core::Interface,
    Win32::Graphics::{
        Dxgi::{
            CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
            IDXGIOutputDuplication, DXGI_DESCRIBE_LAYERED_SURFACE, DXGI_FRAME_STATISTICS,
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_SHAPE,
            DXGI_OUTDUPL_POINTER_SHAPE_MONOCHROME, DXGI_OUTDUPL_POINTER_SHAPE_COLOR,
            DXGI_OUTDUPL_POINTER_SHAPE_ALPHA,
        },
        Dxgi_Common::DXGI_OUTDUPL_POINTER_POSITION,
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG,
            D3D11_MAP_READ, D3D11_MAP_READ_WRITE, D3D11_MAP_WRITE_DISCARD,
            D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
            ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        },
        Direct3D::{D3D_FEATURE_LEVEL_11_0, D3D11_CREATE_DEVICE},
    },
    Win32::Foundation::{DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, HWND, LRESULT, POINTL},
};

/// DXGI Desktop Duplication 屏幕捕获器
pub struct D3D11ScreenCapture {
    /// DXGI 工厂
    factory: IDXGIFactory1,
    /// 适配器（GPU）
    adapter: IDXGIAdapter1,
    /// 输出（显示器）
    output: IDXGIOutput,
    /// 输出复制对象
    duplication: IDXGIOutputDuplication,
    /// D3D11 设备
    device: ID3D11Device,
    /// D3D11 设备上下文
    context: ID3D11DeviceContext,
    /// 屏幕宽度
    width: u32,
    /// 屏幕高度
    height: u32,
    /// Staging texture（用于读取帧数据）
    staging_texture: Option<ID3D11Texture2D>,
    /// 上次捕获的帧（用于无新帧时返回）
    last_frame: Option<CapturedFrame>,
    /// 帧缓冲区复用池（预分配，避免每次分配）
    frame_buffer: Vec<u8>,
    /// 上一次使用的缓冲区（用于复用）
    last_frame_buffer: Option<Vec<u8>>,
    /// 是否已初始化
    initialized: bool,
}

impl D3D11ScreenCapture {
    /// 创建新的 DXGI 屏幕捕获器
    pub fn new() -> Result<Self> {
        let mut capture = D3D11ScreenCapture {
            factory: Default::default(),
            adapter: Default::default(),
            output: Default::default(),
            duplication: Default::default(),
            device: Default::default(),
            context: Default::default(),
            width: 0,
            height: 0,
            staging_texture: None,
            last_frame: None,
            frame_buffer: Vec::new(),
            last_frame_buffer: None,
            initialized: false,
        };
        
        capture.initialize()?;
        Ok(capture)
    }

    /// 初始化 DXGI 和 D3D11
    fn initialize(&mut self) -> Result<()> {
        // 创建 DXGI 工厂
        self.factory = CreateDXGIFactory1::<IDXGIFactory1>()
            .context("Failed to create DXGI factory")?;

        // 获取第一个适配器（GPU）
        self.adapter = self
            .factory
            .GetAdapter(0)
            .context("Failed to get DXGI adapter")?;

        // 获取第一个输出（显示器）
        self.output = self
            .adapter
            .GetOutput(0)
            .context("Failed to get DXGI output")?;

        // 获取输出描述
        let output_desc = self.output.GetDesc().context("Failed to get output description")?;
        self.width = output_desc.DesktopCoordinates.right - output_desc.DesktopCoordinates.left;
        self.height = output_desc.DesktopCoordinates.bottom - output_desc.DesktopCoordinates.top;

        tracing::info!(
            "Screen dimensions: {}x{}",
            self.width,
            self.height
        );

        // 创建 D3D11 设备
        let mut feature_levels = [D3D_FEATURE_LEVEL_11_0];
        let device_result = unsafe {
            D3D11_CREATE_DEVICE(
                Some(&self.adapter),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                &mut feature_levels,
                None,
                None,
            )
        };
        self.device = device_result.context("Failed to create D3D11 device")?;
        self.context = self.device.GetImmediateContext();

        // 创建 staging texture
        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: self.width,
            Height: self.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: windows::Win32::Graphics::Dxgi_Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_MAP_READ,
            MiscFlags: 0,
        };

        self.staging_texture = Some(
            self.device
                .CreateTexture2D(&texture_desc, None)
                .context("Failed to create staging texture")?,
        );

        // 创建输出复制
        self.duplication = self
            .output
            .DuplicateOutput(&self.device)
            .context("Failed to create output duplication")?;

        // 预分配帧缓冲区（BGRA: 4 字节/像素）
        let buffer_size = (self.width * self.height * 4) as usize;
        self.frame_buffer = Vec::with_capacity(buffer_size);
        self.last_frame_buffer = Some(Vec::with_capacity(buffer_size));

        tracing::info!(
            "Pre-allocated frame buffer: {} bytes ({:.2} MB)",
            buffer_size,
            buffer_size as f64 / (1024.0 * 1024.0)
        );

        self.initialized = true;
        tracing::info!("DXGI screen capture initialized successfully");

        Ok(())
    }

    /// 重新初始化（处理 DXGI_ERROR_ACCESS_LOST）
    fn reinitialize(&mut self) -> Result<()> {
        tracing::warn!("Reinitializing DXGI screen capture...");
        
        // 释放现有资源
        self.duplication = Default::default();
        self.staging_texture = None;
        self.initialized = false;

        // 重新初始化
        self.initialize()?;

        Ok(())
    }

    /// 获取当前时间戳（微秒）
    fn get_timestamp_us() -> u64 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        start.elapsed().as_micros() as u64
    }
}

impl ScreenCapture for D3D11ScreenCapture {
    fn capture_frame(&mut self) -> Result<CapturedFrame> {
        if !self.initialized {
            tracing::warn!("Capture called before initialization, attempting to initialize...");
            self.initialize()?;
        }

        // 尝试获取新帧
        let mut frame_info: DXGI_FRAME_STATISTICS = Default::default();
        let mut desktop_image: Option<ID3D11Texture2D> = None;

        match unsafe {
            self.duplication.AcquireNextFrame(
                100, // 超时 100ms
                &mut frame_info,
                &mut desktop_image,
            )
        } {
            Ok(_) => {
                tracing::debug!("Acquired new frame");
            }
            Err(e) if e == DXGI_ERROR_WAIT_TIMEOUT => {
                // 无新帧，返回上一帧
                tracing::debug!("No new frame available, returning last frame");
                if let Some(ref last_frame) = self.last_frame {
                    return Ok(last_frame.clone());
                }
                // 如果没有上一帧，创建一个空白帧
                return self.create_blank_frame();
            }
            Err(e) if e == DXGI_ERROR_ACCESS_LOST => {
                // 需要重新初始化
                tracing::warn!("DXGI_ERROR_ACCESS_LOST, reinitializing...");
                self.reinitialize()?;
                return self.capture_frame(); // 重试
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to acquire frame: {:?}", e));
            }
        }

        let desktop_image = desktop_image.context("Desktop image is null")?;

        // 复制帧到 staging texture
        unsafe {
            self.context.CopyResource(
                self.staging_texture.as_ref().unwrap(),
                &desktop_image,
            );
        }

        // 映射 staging texture 读取数据
        let mapped_resource = unsafe {
            self.context
                .Map(
                    self.staging_texture.as_ref().unwrap().as_ref(),
                    0,
                    D3D11_MAP_READ,
                    0,
                )
                .context("Failed to map staging texture")?
        };

        // 复用帧缓冲区，避免每次分配
        let buffer_size = (self.width * self.height * 4) as usize;
        let row_pitch = mapped_resource.RowPitch as usize;
        let ptr = mapped_resource.pData as *const u8;

        // 确保缓冲区足够大
        if self.frame_buffer.len() < buffer_size {
            self.frame_buffer.resize(buffer_size, 0);
        }

        // 使用 memcpy 方式高效复制行数据
        for y in 0..self.height as usize {
            let row_start = y * row_pitch;
            let dst_start = y * (self.width * 4) as usize;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    ptr.add(row_start),
                    self.frame_buffer.as_mut_ptr().add(dst_start),
                    (self.width * 4) as usize,
                );
            }
        }

        unsafe {
            self.context.Unmap(
                self.staging_texture.as_ref().unwrap().as_ref(),
                0,
            );
        }

        // 释放桌面图像
        unsafe {
            self.duplication.ReleaseFrame()?;
        }

        // 复用 last_frame_buffer 避免重复分配
        let frame_data = std::mem::replace(&mut self.frame_buffer, self.last_frame_buffer.take().unwrap_or_default());
        self.last_frame_buffer = Some(frame_data.clone());

        let frame = CapturedFrame {
            width: self.width,
            height: self.height,
            data: frame_data,
            stride: self.width * 4,
            timestamp_us: Self::get_timestamp_us(),
        };

        self.last_frame = Some(frame.clone());
        Ok(frame)
    }

    fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl D3D11ScreenCapture {
    /// 创建空白帧（用于无上一帧时）
    fn create_blank_frame(&self) -> Result<CapturedFrame> {
        let data = vec![0u8; (self.width * self.height * 4) as usize];
        Ok(CapturedFrame {
            width: self.width,
            height: self.height,
            data,
            stride: self.width * 4,
            timestamp_us: Self::get_timestamp_us(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::screen::CapturedFrame;

    #[test]
    fn test_captured_frame_struct() {
        // 测试 CapturedFrame 结构体可以正确创建
        let frame = CapturedFrame {
            width: 1920,
            height: 1080,
            data: vec![0u8; 1920 * 1080 * 4],
            stride: 1920 * 4,
            timestamp_us: 123456789,
        };
        
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.data.len(), 1920 * 1080 * 4);
        assert_eq!(frame.stride, 1920 * 4);
        assert_eq!(frame.timestamp_us, 123456789);
    }

    #[test]
    fn test_captured_frame_clone() {
        let frame = CapturedFrame {
            width: 1280,
            height: 720,
            data: vec![42u8; 1280 * 720 * 4],
            stride: 1280 * 4,
            timestamp_us: 1000000,
        };
        
        let cloned = frame.clone();
        assert_eq!(frame.width, cloned.width);
        assert_eq!(frame.height, cloned.height);
        assert_eq!(frame.data, cloned.data);
        assert_eq!(frame.stride, cloned.stride);
        assert_eq!(frame.timestamp_us, cloned.timestamp_us);
    }

    #[test]
    fn test_captured_frame_debug() {
        let frame = CapturedFrame {
            width: 1920,
            height: 1080,
            data: vec![],
            stride: 1920 * 4,
            timestamp_us: 0,
        };
        
        let debug_output = format!("{:?}", frame);
        assert!(debug_output.contains("1920"));
        assert!(debug_output.contains("1080"));
    }

    #[cfg(target_os = "windows")]
    mod windows_tests {
        use super::*;
        use crate::screen::dxgi::D3D11ScreenCapture;
        use crate::screen::ScreenCapture;

        #[test]
        fn test_captured_frame_has_valid_dimensions() {
            let mut capture = D3D11ScreenCapture::new().unwrap();
            let (width, height) = capture.get_dimensions();
            
            // 屏幕尺寸应该大于0
            assert!(width > 0);
            assert!(height > 0);
        }

        #[test]
        fn test_capture_frame_returns_bgra_data() {
            let mut capture = D3D11ScreenCapture::new().unwrap();
            let frame = capture.capture_frame().unwrap();
            
            // 检查帧数据格式
            assert!(frame.width > 0);
            assert!(frame.height > 0);
            assert!(frame.stride > 0);
            assert!(!frame.data.is_empty());
            
            // BGRA 格式: 每个像素 4 字节
            let expected_size = (frame.width * frame.height * 4) as usize;
            assert_eq!(frame.data.len(), expected_size);
            
            // 检查时间戳
            assert!(frame.timestamp_us > 0);
        }

        #[test]
        fn test_capture_frame_consistency() {
            let mut capture = D3D11ScreenCapture::new().unwrap();
            
            let (width1, height1) = capture.get_dimensions();
            let frame1 = capture.capture_frame().unwrap();
            
            let (width2, height2) = capture.get_dimensions();
            let frame2 = capture.capture_frame().unwrap();
            
            // 维度应该保持一致
            assert_eq!(width1, width2);
            assert_eq!(height1, height2);
            assert_eq!(frame1.width, frame2.width);
            assert_eq!(frame1.height, frame2.height);
        }
    }
}
