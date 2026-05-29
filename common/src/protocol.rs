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
