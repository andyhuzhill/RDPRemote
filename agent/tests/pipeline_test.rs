//! 视频管道集成测试
//!
//! 测试视频编码、自适应码率、ROI 检测和帧跳过等核心组件

use rdp_agent::encoder::{VP9Encoder, VideoEncoder};
use rdp_agent::roi::RoiDetector;
use rdp_agent::adaptive::{AdaptiveController, BandwidthTier};
use rdp_agent::frame_skipper::FrameSkipper;

/// 创建测试帧 (BGRA 格式)
fn create_test_frame(width: u32, height: u32) -> Vec<u8> {
    vec![128u8; (width * height * 4) as usize]
}

/// VP9 编码测试 - 由于 libvpx 1.16 的 gf_frame_max_boost_factor 字段问题，
/// 此测试在 Linux 上暂时被忽略。需要在 Windows 上或使用兼容的 libvpx 版本运行。
#[test]
#[ignore]
fn test_vp9_encode_decode() {
    let width = 640u32;
    let height = 480u32;
    let mut encoder = VP9Encoder::new(width, height, 500).unwrap();
    
    let frame = create_test_frame(width, height);
    let encoded = encoder.encode(&frame, width, height, 0).unwrap();
    
    assert!(!encoded.data.is_empty());
    assert_eq!(encoded.width, width);
    assert_eq!(encoded.height, height);
}

#[test]
fn test_adaptive_bitrate_control() {
    let mut controller = AdaptiveController::new();
    
    // 初始层级应该是 Medium
    assert_eq!(controller.current_tier(), BandwidthTier::Medium);
    
    // 模拟高带宽
    controller.add_bytes_sent(300_000); // 300KB
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    let tier = controller.check_and_adjust();
    // 应该检测到带宽变化
    assert!(tier.is_some());
}

#[test]
fn test_roi_detection() {
    let width = 640u32;
    let height = 480u32;
    let mut roi = RoiDetector::new(width, height, 32);
    
    let frame1 = create_test_frame(width, height);
    let changes1 = roi.detect_changes(&frame1, width, height);
    // 第一帧应该检测到变化（prev_hashes 初始为 0，新帧 hash 不为 0）
    assert!(!changes1.is_empty());
    
    let frame2 = create_test_frame(width, height);
    let changes2 = roi.detect_changes(&frame2, width, height);
    // 相同帧应该没有变化
    assert!(changes2.is_empty());
}

#[test]
fn test_frame_skipper() {
    let mut skipper = FrameSkipper::new(30);
    
    // 有变化时不跳过
    assert!(!skipper.should_skip(true));
    assert_eq!(skipper.target_fps(), 30);
    
    // 连续无变化后跳过
    for _ in 0..4 {
        assert!(!skipper.should_skip(false));
    }
    assert!(skipper.should_skip(false));
    assert_eq!(skipper.target_fps(), 5);
}
