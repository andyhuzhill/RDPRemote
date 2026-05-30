use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFrameHeader {
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
    pub codec: VideoCodec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VideoCodec {
    VP9,
    H264,
}

/// 零拷贝编码帧，使用 Bytes 避免 Vec<u8> 复制
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Bytes,
    pub header: VideoFrameHeader,
}

impl EncodedFrame {
    /// 创建新的编码帧
    pub fn new(data: Bytes, header: VideoFrameHeader) -> Self {
        Self { data, header }
    }
    
    /// 从 Vec<u8> 创建编码帧（会转移所有权，零拷贝）
    pub fn from_vec(data: Vec<u8>, header: VideoFrameHeader) -> Self {
        Self {
            data: Bytes::from(data),
            header,
        }
    }
    
    /// 获取帧数据的引用
    pub fn data(&self) -> &Bytes {
        &self.data
    }
    
    /// 获取帧长度
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// 判断帧是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
