//! 带宽自适应模块

use std::time::{Duration, Instant};

/// 带宽层级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthTier {
    High,      // > 2 Mbps: 1080p @ 30fps
    Medium,    // 1-2 Mbps: 720p @ 15fps
    Low,       // 500K-1Mbps: 720p @ 10fps
    VeryLow,   // < 500K: 480p @ 8fps
}

impl BandwidthTier {
    pub fn width(&self) -> u32 {
        match self {
            Self::High => 1920,
            Self::Medium | Self::Low => 1280,
            Self::VeryLow => 800,
        }
    }
    
    pub fn height(&self) -> u32 {
        match self {
            Self::High => 1080,
            Self::Medium | Self::Low => 720,
            Self::VeryLow => 600,
        }
    }
    
    pub fn fps(&self) -> u32 {
        match self {
            Self::High => 30,
            Self::Medium => 15,
            Self::Low => 10,
            Self::VeryLow => 8,
        }
    }
    
    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            Self::High => 2000,
            Self::Medium => 1000,
            Self::Low => 600,
            Self::VeryLow => 300,
        }
    }
    
    pub fn frame_interval(&self) -> Duration {
        Duration::from_millis(1000 / self.fps() as u64)
    }
}

/// 带宽自适应控制器
pub struct AdaptiveController {
    current_tier: BandwidthTier,
    last_check: Instant,
    check_interval: Duration,
    // 统计信息
    bytes_sent: u64,
    last_bytes: u64,
}

impl AdaptiveController {
    pub fn new() -> Self {
        Self {
            current_tier: BandwidthTier::Medium,
            last_check: Instant::now(),
            check_interval: Duration::from_secs(2),
            bytes_sent: 0,
            last_bytes: 0,
        }
    }
    
    pub fn new_with_interval(check_interval: Duration) -> Self {
        Self {
            current_tier: BandwidthTier::Medium,
            last_check: Instant::now(),
            check_interval,
            bytes_sent: 0,
            last_bytes: 0,
        }
    }
    
    pub fn current_tier(&self) -> BandwidthTier {
        self.current_tier
    }
    
    pub fn add_bytes_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
    }
    
    /// 检查并调整带宽层级
    pub fn check_and_adjust(&mut self) -> Option<BandwidthTier> {
        if self.last_check.elapsed() < self.check_interval {
            return None;
        }
        
        let elapsed = self.last_check.elapsed().as_secs_f64();
        let bytes_in_period = self.bytes_sent - self.last_bytes;
        let current_bps = (bytes_in_period as f64 * 8.0) / elapsed;
        let current_kbps = current_bps / 1000.0;
        
        let new_tier = if current_kbps > 2000.0 {
            BandwidthTier::High
        } else if current_kbps > 1000.0 {
            BandwidthTier::Medium
        } else if current_kbps > 500.0 {
            BandwidthTier::Low
        } else {
            BandwidthTier::VeryLow
        };
        
        self.last_check = Instant::now();
        self.last_bytes = self.bytes_sent;
        
        if new_tier != self.current_tier {
            self.current_tier = new_tier;
            Some(new_tier)
        } else {
            None
        }
    }
    
    /// 强制检查（用于测试）
    pub fn force_check(&mut self) -> Option<BandwidthTier> {
        let elapsed = self.check_interval.as_secs_f64();
        if elapsed <= 0.0 {
            // 如果间隔为0，则不更新 last_bytes，以便累积
            return None;
        }
        let bytes_in_period = self.bytes_sent - self.last_bytes;
        let current_bps = (bytes_in_period as f64 * 8.0) / elapsed;
        let current_kbps = current_bps / 1000.0;
        
        let new_tier = if current_kbps > 2000.0 {
            BandwidthTier::High
        } else if current_kbps > 1000.0 {
            BandwidthTier::Medium
        } else if current_kbps > 500.0 {
            BandwidthTier::Low
        } else {
            BandwidthTier::VeryLow
        };
        
        self.last_bytes = self.bytes_sent;
        
        if new_tier != self.current_tier {
            self.current_tier = new_tier;
            Some(new_tier)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_tier_resolution() {
        // 使用 2s 间隔
        let mut controller = AdaptiveController::new_with_interval(Duration::from_secs(2));
        
        // 初始为 Medium
        assert_eq!(controller.current_tier(), BandwidthTier::Medium);
        
        // 模拟高带宽 (> 2 Mbps)
        controller.add_bytes_sent(600_000); // 2s * 2Mbps = 4Mbits = 500KB
        let tier = controller.force_check();
        assert_eq!(tier, Some(BandwidthTier::High));
        assert_eq!(controller.current_tier(), BandwidthTier::High);
        
        // 模拟中等带宽 (1-2 Mbps)
        controller.add_bytes_sent(300_000); // 2s * 1.2Mbps = 2.4Mbits = 300KB
        let tier = controller.force_check();
        assert_eq!(tier, Some(BandwidthTier::Medium));
        
        // 模拟低带宽 (500K-1Mbps)
        controller.add_bytes_sent(150_000); // 2s * 600Kbps = 1.2Mbits = 150KB
        let tier = controller.force_check();
        assert_eq!(tier, Some(BandwidthTier::Low));
        
        // 模拟极低带宽 (< 500K)
        controller.add_bytes_sent(75_000); // 2s * 300Kbps = 600Kbits = 75KB
        let tier = controller.force_check();
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
        let mut controller = AdaptiveController::new_with_interval(Duration::from_millis(100));
        
        // 在检查间隔内调用，应返回 None
        let tier = controller.check_and_adjust();
        assert_eq!(tier, None);
        
        // 等待后调用，应返回结果
        std::thread::sleep(Duration::from_millis(150));
        controller.add_bytes_sent(600_000);
        let tier = controller.check_and_adjust();
        assert_eq!(tier, Some(BandwidthTier::High));
    }
}
