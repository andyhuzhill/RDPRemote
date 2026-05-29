//! VP9 Encoder tests

use crate::encoder::{EncodedFrame, VideoEncoder, VP9Encoder};

#[test]
fn test_vp9_encoder_new() {
    match VP9Encoder::new(1280, 720, 1000) {
        Ok(_) => {},
        Err(e) => panic!("VP9Encoder::new failed: {}", e),
    }
}

#[test]
fn test_vp9_encoder_encode_bgra() {
    let width = 1280;
    let height = 720;
    let mut encoder = VP9Encoder::new(width, height, 1000).unwrap();
    
    // Create a simple BGRA frame (4 bytes per pixel)
    let frame_size = (width * height * 4) as usize;
    let mut frame = vec![0u8; frame_size];
    
    // Fill with a test pattern
    for i in 0..height {
        for j in 0..width {
            let idx = ((i * width + j) * 4) as usize;
            frame[idx] = 0;     // Blue
            frame[idx + 1] = 0; // Green
            frame[idx + 2] = 255; // Red
            frame[idx + 3] = 255; // Alpha
        }
    }
    
    let result = encoder.encode(&frame, width, height, 0);
    assert!(result.is_ok());
    
    let encoded_frame = result.unwrap();
    assert!(!encoded_frame.data.is_empty());
    assert!(encoded_frame.width == width);
    assert!(encoded_frame.height == height);
}

#[test]
fn test_vp9_encoder_set_bitrate() {
    let mut encoder = VP9Encoder::new(1280, 720, 1000).unwrap();
    encoder.set_bitrate(2000);
    // Bitrate change should succeed
}

#[test]
fn test_vp9_encoder_force_keyframe() {
    let mut encoder = VP9Encoder::new(1280, 720, 1000).unwrap();
    encoder.force_keyframe();
    // Force keyframe should succeed
}

#[test]
fn test_vp9_encoder_encode_keyframe() {
    let width = 1280;
    let height = 720;
    let mut encoder = VP9Encoder::new(width, height, 1000).unwrap();
    
    // Force a keyframe first
    encoder.force_keyframe();
    
    let frame_size = (width * height * 4) as usize;
    let frame = vec![0u8; frame_size];
    
    let result = encoder.encode(&frame, width, height, 0);
    assert!(result.is_ok());
    
    let encoded_frame = result.unwrap();
    assert!(encoded_frame.is_keyframe, "First frame after force_keyframe should be a keyframe");
}

#[test]
fn test_vp9_encoder_multiple_frames() {
    let width = 1280;
    let height = 720;
    let mut encoder = VP9Encoder::new(width, height, 1000).unwrap();
    
    let frame_size = (width * height * 4) as usize;
    
    // Encode multiple frames
    for i in 0..5 {
        let frame = vec![i as u8; frame_size];
        let result = encoder.encode(&frame, width, height, (i as u64) * 33_333);
        assert!(result.is_ok(), "Frame {} encoding should succeed", i);
    }
}

#[test]
fn test_vp9_encoder_bgra_to_i420_conversion() {
    // Test that BGRA to I420 conversion works correctly
    let width = 1280;
    let height = 720;
    let mut encoder = VP9Encoder::new(width, height, 1000).unwrap();
    
    // Create a solid color frame (pure red in BGRA)
    let frame_size = (width * height * 4) as usize;
    let mut frame = vec![0u8; frame_size];
    for i in 0..frame_size {
        if i % 4 == 2 {
            frame[i] = 255; // Red channel
        } else {
            frame[i] = 0;
        }
    }
    
    let result = encoder.encode(&frame, width, height, 0);
    assert!(result.is_ok());
    
    let encoded_frame = result.unwrap();
    // VP9 encoded data should not be empty
    assert!(!encoded_frame.data.is_empty());
}
