/// 帧跳过控制器
pub struct FrameSkipper {
    unchanged_count: u32,
    skip_threshold: u32,      // 连续无变化帧数阈值
    current_fps: u32,
    min_fps: u32,
    max_fps: u32,
}

impl FrameSkipper {
    pub fn new(max_fps: u32) -> Self {
        Self {
            unchanged_count: 0,
            skip_threshold: 5,
            current_fps: max_fps,
            min_fps: 5,
            max_fps,
        }
    }
    
    /// 报告帧是否有变化，返回是否应该跳过本帧
    pub fn should_skip(&mut self, has_changes: bool) -> bool {
        if has_changes {
            self.unchanged_count = 0;
            self.current_fps = self.max_fps;
            false
        } else {
            self.unchanged_count += 1;
            if self.unchanged_count >= self.skip_threshold {
                self.current_fps = self.min_fps;
                true
            } else {
                false
            }
        }
    }
    
    /// 获取当前目标帧率
    pub fn target_fps(&self) -> u32 {
        self.current_fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_frame_skipper_no_skip() {
        let mut skipper = FrameSkipper::new(30);
        assert!(!skipper.should_skip(true));
        assert_eq!(skipper.target_fps(), 30);
    }
    
    #[test]
    fn test_frame_skipper_skip_after_threshold() {
        let mut skipper = FrameSkipper::new(30);
        for _ in 0..4 {
            assert!(!skipper.should_skip(false));
        }
        assert!(skipper.should_skip(false));
        assert_eq!(skipper.target_fps(), 5);
    }
}
