//! 屏幕捕获测试

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

    #[tokio::test]
    async fn test_captured_frame_has_valid_dimensions() {
        let mut capture = D3D11ScreenCapture::new().await.unwrap();
        let (width, height) = capture.get_dimensions();
        
        // 屏幕尺寸应该大于0
        assert!(width > 0);
        assert!(height > 0);
    }

    #[tokio::test]
    async fn test_capture_frame_returns_bgra_data() {
        let mut capture = D3D11ScreenCapture::new().await.unwrap();
        let frame = capture.capture_frame().await.unwrap();
        
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

    #[tokio::test]
    async fn test_capture_frame_consistency() {
        let mut capture = D3D11ScreenCapture::new().await.unwrap();
        
        let (width1, height1) = capture.get_dimensions();
        let frame1 = capture.capture_frame().await.unwrap();
        
        let (width2, height2) = capture.get_dimensions();
        let frame2 = capture.capture_frame().await.unwrap();
        
        // 维度应该保持一致
        assert_eq!(width1, width2);
        assert_eq!(height1, height2);
        assert_eq!(frame1.width, frame2.width);
        assert_eq!(frame1.height, frame2.height);
    }
}
