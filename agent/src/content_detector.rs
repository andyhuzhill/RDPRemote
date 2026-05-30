//! 屏幕内容变化检测器
//!
//! 采用采样策略，只检查部分区域（tile），降低计算开销。
//! 作为独立层，与带宽自适应并行工作。

use std::time::{Duration, Instant};

/// 内容变化类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentChange {
    /// 无变化
    NoChange,
    /// 检测到变化
    Active,
}

/// 内容检测器
///
/// 使用固定阈值和采样策略来检测屏幕内容变化。
/// 独立于带宽自适应层工作，提供原始变化信号。
pub struct ContentDetector {
    /// 采样率：每 N 个 tile 检查一个（0 = 全部检查）
    sample_rate: u32,
    /// 变化阈值：连续 N 帧无变化才判定为静止
    quiet_threshold: u32,
    /// 当前连续无变化帧数
    unchanged_count: u32,
    /// 上次检测时间
    last_check: Instant,
    /// 检测间隔
    check_interval: Duration,
    /// 当前状态
    current_state: ContentChange,
}

impl ContentDetector {
    /// 创建新的内容检测器
    ///
    /// # 参数
    /// * `sample_rate` - 采样率，0 表示不采样（检查所有 tile）
    /// * `quiet_threshold` - 静止判定阈值（连续无变化帧数）
    /// * `check_interval` - 检测间隔
    pub fn new(sample_rate: u32, quiet_threshold: u32, check_interval: Duration) -> Self {
        Self {
            sample_rate,
            quiet_threshold,
            unchanged_count: 0,
            last_check: Instant::now(),
            check_interval,
            current_state: ContentChange::NoChange,
        }
    }

    /// 获取当前内容状态
    pub fn current_state(&self) -> ContentChange {
        self.current_state
    }

    /// 报告一次检测结果
    ///
    /// # 参数
    /// * `has_changes` - 本次采样是否检测到变化
    pub fn report(&mut self, has_changes: bool) {
        if has_changes {
            self.unchanged_count = 0;
            self.current_state = ContentChange::Active;
        } else {
            self.unchanged_count += 1;
            if self.unchanged_count >= self.quiet_threshold {
                self.current_state = ContentChange::NoChange;
            }
        }
    }

    /// 检查并更新状态
    ///
    /// 返回是否需要重新检测（基于时间间隔）
    pub fn check_and_update(&mut self) -> bool {
        if self.last_check.elapsed() < self.check_interval {
            return false;
        }

        self.last_check = Instant::now();
        true
    }

    /// 强制更新（用于测试）
    pub fn force_update(&mut self, has_changes: bool) {
        self.report(has_changes);
    }

    /// 重置检测器状态
    pub fn reset(&mut self) {
        self.unchanged_count = 0;
        self.current_state = ContentChange::NoChange;
    }

    /// 获取采样率
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 获取静止阈值
    pub fn quiet_threshold(&self) -> u32 {
        self.quiet_threshold
    }
}

impl Default for ContentDetector {
    fn default() -> Self {
        Self::new(4, 3, Duration::from_millis(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_detector_active() {
        let mut detector = ContentDetector::default();
        
        // 检测到变化
        detector.force_update(true);
        assert_eq!(detector.current_state(), ContentChange::Active);
    }

    #[test]
    fn test_content_detector_quiet() {
        let mut detector = ContentDetector::new(0, 3, Duration::from_millis(100));
        
        // 连续无变化，达到阈值后进入静止状态
        detector.force_update(false);
        assert_eq!(detector.current_state(), ContentChange::Active);
        
        detector.force_update(false);
        assert_eq!(detector.current_state(), ContentChange::Active);
        
        detector.force_update(false);
        assert_eq!(detector.current_state(), ContentChange::NoChange);
    }

    #[test]
    fn test_content_detector_sample_rate() {
        let detector = ContentDetector::new(4, 3, Duration::from_millis(100));
        assert_eq!(detector.sample_rate(), 4);
    }

    #[test]
    fn test_content_detector_reset() {
        let mut detector = ContentDetector::new(0, 3, Duration::from_millis(100));
        
        detector.force_update(false);
        detector.force_update(false);
        detector.force_update(false);
        assert_eq!(detector.current_state(), ContentChange::NoChange);
        
        detector.reset();
        assert_eq!(detector.current_state(), ContentChange::NoChange);
        assert_eq!(detector.unchanged_count, 0);
    }

    #[test]
    fn test_check_interval() {
        let mut detector = ContentDetector::new(0, 3, Duration::from_millis(50));
        
        // 在间隔内，返回 false
        assert!(!detector.check_and_update());
        
        // 等待后，返回 true
        std::thread::sleep(Duration::from_millis(60));
        assert!(detector.check_and_update());
    }
}
