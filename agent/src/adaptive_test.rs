#[cfg(test)]
mod tests {
    use rdp_agent::adaptive::{AdaptiveController, BandwidthTier};
    use std::time::Duration;

    #[test]
    fn test_bandwidth_tier_resolution() {
        let mut controller = AdaptiveController::new();
        
        // 初始为 Medium
        assert_eq!(controller.current_tier(), BandwidthTier::Medium);
        
        // 模拟高带宽 (> 2 Mbps)
        controller.add_bytes_sent(600_000); // 2s * 2Mbps = 4Mbits = 500KB
        let tier = controller.check_and_adjust();
        assert_eq!(tier, Some(BandwidthTier::High));
        assert_eq!(controller.current_tier(), BandwidthTier::High);
        
        // 模拟中等带宽 (1-2 Mbps)
        controller.add_bytes_sent(300_000); // 2s * 1.2Mbps = 2.4Mbits = 300KB
        let tier = controller.check_and_adjust();
        assert_eq!(tier, Some(BandwidthTier::Medium));
        
        // 模拟低带宽 (500K-1Mbps)
        controller.add_bytes_sent(150_000); // 2s * 600Kbps = 1.2Mbits = 150KB
        let tier = controller.check_and_adjust();
        assert_eq!(tier, Some(BandwidthTier::Low));
        
        // 模拟极低带宽 (< 500K)
        controller.add_bytes_sent(75_000); // 2s * 300Kbps = 600Kbits = 75KB
        let tier = controller.check_and_adjust();
        assert_eq!(tier, Some(BandwidthTier::VeryLow));
    }

    #[test]
    fn test_bandwidth_tier_parameters() {
        assert_eq!(BandwidthTier::High.width(), 1920);
        assert_eq!(BandwidthTier::High.height(), 1080);
        assert_eq!(BandwidthTier::High.fps(), 30);
        assert_eq!(BandwidthTier::High.bitrate_kbps(), 2000);
        assert_eq!(BandwidthTier::High.frame_interval(), Duration::from_millis(33));
        
        assert_eq!(BandwidthTier::Medium.width(), 1280);
        assert_eq!(BandwidthTier::Medium.height(), 720);
        assert_eq!(BandwidthTier::Medium.fps(), 15);
        assert_eq!(BandwidthTier::Medium.bitrate_kbps(), 1000);
        assert_eq!(BandwidthTier::Medium.frame_interval(), Duration::from_millis(66));
        
        assert_eq!(BandwidthTier::Low.width(), 1280);
        assert_eq!(BandwidthTier::Low.height(), 720);
        assert_eq!(BandwidthTier::Low.fps(), 10);
        assert_eq!(BandwidthTier::Low.bitrate_kbps(), 600);
        assert_eq!(BandwidthTier::Low.frame_interval(), Duration::from_millis(100));
        
        assert_eq!(BandwidthTier::VeryLow.width(), 800);
        assert_eq!(BandwidthTier::VeryLow.height(), 600);
        assert_eq!(BandwidthTier::VeryLow.fps(), 8);
        assert_eq!(BandwidthTier::VeryLow.bitrate_kbps(), 300);
        assert_eq!(BandwidthTier::VeryLow.frame_interval(), Duration::from_millis(125));
    }

    #[test]
    fn test_check_interval() {
        let mut controller = AdaptiveController::new();
        
        // 在检查间隔内调用，应返回 None
        let tier = controller.check_and_adjust();
        assert_eq!(tier, None);
    }
}
