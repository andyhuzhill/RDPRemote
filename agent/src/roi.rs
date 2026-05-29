//! ROI (Region of Interest) 变化检测模块
//!
//! 使用 tile hash 方案检测屏幕变化区域：
//! 1. 将屏幕分成 32x32 或 64x64 的 tile
//! 2. 计算每个 tile 的 hash（简单的像素和）
//! 3. 与上一帧的 hash 比较，找出变化的 tile
//! 4. 只编码变化区域，节省带宽

/// 变化区域
#[derive(Debug, Clone, PartialEq)]
pub struct TileRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// ROI (Region of Interest) 变化检测器
pub struct RoiDetector {
    tile_size: u32,
    tiles_x: u32,
    tiles_y: u32,
    prev_hashes: Vec<u64>,
}

impl RoiDetector {
    /// 创建新的 ROI 检测器
    ///
    /// # Arguments
    /// * `width` - 屏幕宽度
    /// * `height` - 屏幕高度
    /// * `tile_size` - tile 大小（像素），建议 32 或 64
    pub fn new(width: u32, height: u32, tile_size: u32) -> Self {
        let tiles_x = (width + tile_size - 1) / tile_size;
        let tiles_y = (height + tile_size - 1) / tile_size;
        
        RoiDetector {
            tile_size,
            tiles_x,
            tiles_y,
            prev_hashes: vec![0; (tiles_x * tiles_y) as usize],
        }
    }
    
    /// 计算 tile 的 hash 值
    /// 使用简单的像素和 hash 算法
    pub(crate) fn compute_tile_hash(tile: &[u8]) -> u64 {
        let mut hash: u64 = 0;
        for chunk in tile.chunks(4) {
            if chunk.len() < 4 {
                continue;
            }
            let pixel = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            hash = hash.wrapping_add(pixel as u64);
        }
        hash
    }
    
    /// 检测变化区域，返回变化的 tile 坐标列表
    ///
    /// # Arguments
    /// * `frame` - 当前帧的像素数据 (BGRA 格式，每像素 4 字节)
    /// * `width` - 帧宽度
    /// * `height` - 帧高度
    ///
    /// # Returns
    /// 变化区域的 tile 列表，相邻变化 tile 会被合并
    pub fn detect_changes(&mut self, frame: &[u8], width: u32, height: u32) -> Vec<TileRegion> {
        let frame_size = (width * height * 4) as usize;
        if frame.len() < frame_size {
            return Vec::new();
        }
        
        // 收集所有变化的 tile
        let mut changed_tiles: Vec<(u32, u32)> = Vec::new();
        
        for tile_y in 0..self.tiles_y {
            for tile_x in 0..self.tiles_x {
                // 计算 tile 的像素区域
                let tile_x_start = tile_x * self.tile_size;
                let tile_y_start = tile_y * self.tile_size;
                let tile_x_end = std::cmp::min(tile_x_start + self.tile_size, width);
                let tile_y_end = std::cmp::min(tile_y_start + self.tile_size, height);
                
                let tile_width = tile_x_end - tile_x_start;
                let tile_height = tile_y_end - tile_y_start;
                
                // 提取 tile 数据
                let mut tile_data: Vec<u8> = Vec::with_capacity((tile_width * tile_height * 4) as usize);
                for row in 0..tile_height {
                    let row_start = ((tile_y_start + row) * width + tile_x_start) as usize * 4;
                    tile_data.extend_from_slice(&frame[row_start..row_start + (tile_width * 4) as usize]);
                }
                
                // 计算当前 hash
                let current_hash = Self::compute_tile_hash(&tile_data);
                
                // 获取 tile 索引
                let tile_idx = (tile_y * self.tiles_x + tile_x) as usize;
                
                // 比较 hash
                if self.prev_hashes[tile_idx] != current_hash {
                    changed_tiles.push((tile_x, tile_y));
                }
                
                // 更新 prev_hashes
                self.prev_hashes[tile_idx] = current_hash;
            }
        }
        
        // 合并相邻的变化 tile
        Self::merge_tiles(changed_tiles, self.tile_size)
    }
    
    /// 合并相邻的变化 tile 为更大的矩形区域
    fn merge_tiles(tiles: Vec<(u32, u32)>, tile_size: u32) -> Vec<TileRegion> {
        if tiles.is_empty() {
            return Vec::new();
        }
        
        // 使用简单的合并策略：找到最小包围矩形
        // 对于更复杂的场景，可以使用更高级的算法（如连通区域分析）
        
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        
        for (tx, ty) in &tiles {
            min_x = min_x.min(*tx);
            min_y = min_y.min(*ty);
            max_x = max_x.max(*tx);
            max_y = max_y.max(*ty);
        }
        
        // 如果所有 tile 是连通的，返回单个合并区域
        // 这里使用简单的启发式：如果 tile 数量较少且密集，合并为一个区域
        if tiles.len() <= 9 {
            // 小区域合并
            return vec![TileRegion {
                x: min_x * tile_size,
                y: min_y * tile_size,
                width: (max_x - min_x + 1) * tile_size,
                height: (max_y - min_y + 1) * tile_size,
            }];
        }
        
        // 对于大区域，返回单个包围矩形
        vec![TileRegion {
            x: min_x * tile_size,
            y: min_y * tile_size,
            width: (max_x - min_x + 1) * tile_size,
            height: (max_y - min_y + 1) * tile_size,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_tile_hash() {
        // 空白 tile (全0)
        let blank = vec![0u8; 16]; // 4x4 像素, 每像素 4 字节
        let hash = RoiDetector::compute_tile_hash(&blank);
        assert_eq!(hash, 0);
        
        // 非空 tile
        let non_blank = vec![0xFFu8; 16];
        let hash = RoiDetector::compute_tile_hash(&non_blank);
        assert!(hash > 0);
    }
    
    #[test]
    fn test_roi_detector_creation() {
        let detector = RoiDetector::new(1920, 1080, 32);
        assert_eq!(detector.tile_size, 32);
        assert_eq!(detector.tiles_x, 60); // 1920 / 32
        assert_eq!(detector.tiles_y, 34); // 1080 / 32 = 33.75, 向上取整
    }
    
    #[test]
    fn test_roi_detector_detection_no_change() {
        let width = 128u32;
        let height = 128u32;
        let tile_size = 32u32;
        
        let mut detector = RoiDetector::new(width, height, tile_size);
        
        // 创建空白帧
        let frame = vec![0u8; (width * height * 4) as usize];
        
        // 第一次检测，由于 prev_hashes 初始为 0，空白帧 hash 也是 0，所以返回空
        let changes = detector.detect_changes(&frame, width, height);
        assert_eq!(changes.len(), 0);
        
        // 第二次检测，应该返回空 (因为 hash 没变)
        let changes = detector.detect_changes(&frame, width, height);
        assert_eq!(changes.len(), 0);
    }
    
    #[test]
    fn test_roi_detector_detection_with_change() {
        let width = 128u32;
        let height = 128u32;
        let tile_size = 32u32;
        
        let mut detector = RoiDetector::new(width, height, tile_size);
        
        // 创建空白帧
        let mut frame = vec![0u8; (width * height * 4) as usize];
        
        // 第一次检测 (初始化 prev_hashes)
        let _ = detector.detect_changes(&frame, width, height);
        
        // 修改一个 tile 的内容 (左上角 tile)
        // 注意：tile 数据在帧中是分散的（行优先存储）
        for row in 0..tile_size {
            let row_start = (row * width) as usize * 4;
            let tile_start = row_start + (0 * 4) as usize; // tile x=0
            for i in 0..(tile_size * 4) as usize {
                frame[tile_start + i] = 0xFF;
            }
        }
        
        // 第二次检测，应该只返回左上角的 tile
        let changes = detector.detect_changes(&frame, width, height);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].x, 0);
        assert_eq!(changes[0].y, 0);
        assert_eq!(changes[0].width, tile_size);
        assert_eq!(changes[0].height, tile_size);
    }
    
    #[test]
    fn test_tile_region_merge() {
        // 测试相邻变化 tile 的合并
        let mut detector = RoiDetector::new(128, 128, 32);
        
        let mut frame = vec![0u8; (128 * 128 * 4) as usize];
        
        // 辅助函数：修改指定 tile 的内容
        fn modify_tile(frame: &mut [u8], width: u32, tile_x: u32, tile_y: u32, tile_size: u32) {
            for row in 0..tile_size {
                let row_start = ((tile_y * tile_size + row) * width) as usize * 4;
                let tile_start = row_start + (tile_x * tile_size * 4) as usize;
                for i in 0..(tile_size * 4) as usize {
                    frame[tile_start + i] = 0xFF;
                }
            }
        }
        
        // 修改左上角 2x2 的 tile
        for row in 0..2 {
            for col in 0..2 {
                modify_tile(&mut frame, 128, col, row, 32);
            }
        }
        
        // 初始化 prev_hashes
        let _ = detector.detect_changes(&frame, 128, 128);
        
        // 修改更多 tile (3x3)
        for row in 0..3 {
            for col in 0..3 {
                modify_tile(&mut frame, 128, col, row, 32);
            }
        }
        
        let changes = detector.detect_changes(&frame, 128, 128);
        // 应该返回 1 个合并后的区域 (3x3 tiles)
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].width, 96); // 3 * 32
        assert_eq!(changes[0].height, 96); // 3 * 32
    }
    
    #[test]
    fn test_roi_detector_edge_cases() {
        // 测试非整除的屏幕尺寸
        let detector = RoiDetector::new(1920, 1080, 32);
        assert_eq!(detector.tiles_x, 60);
        assert_eq!(detector.tiles_y, 34);
        
        // 测试小屏幕
        let detector = RoiDetector::new(64, 64, 32);
        assert_eq!(detector.tiles_x, 2);
        assert_eq!(detector.tiles_y, 2);
        
        // 测试 tile 大小等于屏幕大小
        let detector = RoiDetector::new(64, 64, 64);
        assert_eq!(detector.tiles_x, 1);
        assert_eq!(detector.tiles_y, 1);
    }
}
