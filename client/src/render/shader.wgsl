// WGPU 视频渲染着色器
// 使用全屏四边形渲染视频纹理

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    // 将 NDC 坐标转换为裁剪空间（y 轴反转）
    out.clip_position = vec4<f32>(position.x, -position.y, 0.0, 1.0);
    out.tex_coord = tex_coord;
    return out;
}

@group(0) @binding(0)
var video_texture: texture_2d<f32>;

@group(0) @binding(1)
var video_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    // 采样视频纹理
    // BGRA 数据使用 Rgba8Unorm 格式，直接返回 RGBA
    let color = textureSample(video_texture, video_sampler, tex_coord);
    return color;
}
