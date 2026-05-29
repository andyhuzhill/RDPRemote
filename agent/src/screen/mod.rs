//! 屏幕捕获模块
//!
//! 提供跨平台的屏幕捕获功能。
//! Windows 平台使用 DXGI Desktop Duplication API。

#[cfg(target_os = "windows")]
pub mod dxgi;

/// 捕获的帧数据
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// 帧宽度（像素）
    pub width: u32,
    /// 帧高度（像素）
    pub height: u32,
    /// 帧数据（BGRA 格式，每像素 4 字节）
    pub data: Vec<u8>,
    /// 行跨度（字节数）
    pub stride: u32,
    /// 时间戳（微秒）
    pub timestamp_us: u64,
}

#[cfg(target_os = "windows")]
pub use dxgi::D3D11ScreenCapture;

/// 屏幕捕获 trait，定义了屏幕捕获的标准接口
pub trait ScreenCapture {
    /// 捕获一帧屏幕图像
    /// 
    /// # Returns
    /// - `Ok(CapturedFrame)`: 包含捕获的帧数据
    /// - `Err(anyhow::Error)`: 捕获失败
    fn capture_frame(&mut self) -> anyhow::Result<CapturedFrame>;
    
    /// 获取屏幕维度
    /// 
    /// # Returns
    /// - `(width, height)`: 屏幕的宽度和高度
    fn get_dimensions(&self) -> (u32, u32);
}

