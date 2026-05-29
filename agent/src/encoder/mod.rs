//! Video encoder module

pub mod vp9;

#[cfg(test)]
mod vp9_test;

pub use vp9::VP9Encoder;

/// Encoded video frame
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    pub timestamp_us: u64,
    pub width: u32,
    pub height: u32,
}

/// Video encoder trait
pub trait VideoEncoder {
    /// Encode a BGRA frame
    fn encode(&mut self, frame: &[u8], width: u32, height: u32, timestamp_us: u64) -> anyhow::Result<EncodedFrame>;
    
    /// Set target bitrate in kbps
    fn set_bitrate(&mut self, bitrate_kbps: u32);
    
    /// Force next frame to be a keyframe
    fn force_keyframe(&mut self);
}
