//! common crate Bytes 类型测试

use bytes::Bytes;
use rdp_common::protocol::{EncodedFrame, VideoFrameHeader, VideoCodec};

#[test]
fn test_encoded_frame_clone_shares_data() {
    // 测试 EncodedFrame 克隆时共享底层数据（零拷贝）
    let data = Bytes::from(vec![1u8, 2, 3, 4, 5]);
    let header = VideoFrameHeader {
        width: 1920,
        height: 1080,
        timestamp_us: 0,
        is_keyframe: true,
        codec: VideoCodec::VP9,
    };
    
    let frame1 = EncodedFrame::new(data, header.clone());
    let frame2 = frame1.clone();
    
    // 克隆后数据应该共享（refcnt > 1）
    assert_eq!(frame1.data.len(), frame2.data.len());
    assert_eq!(&frame1.data[..], &frame2.data[..]);
}

#[test]
fn test_bytes_from_vec() {
    let vec_data = vec![10u8, 20, 30];
    let bytes = Bytes::from(vec_data);
    
    assert_eq!(bytes.len(), 3);
    assert_eq!(bytes[0], 10);
    assert_eq!(bytes[1], 20);
    assert_eq!(bytes[2], 30);
}

#[test]
fn test_encoded_frame_from_vec() {
    let data = vec![0u8; 256];
    let header = VideoFrameHeader {
        width: 1280,
        height: 720,
        timestamp_us: 1000,
        is_keyframe: false,
        codec: VideoCodec::H264,
    };
    
    let frame = EncodedFrame::from_vec(data, header);
    
    assert_eq!(frame.data.len(), 256);
    assert!(!frame.header.is_keyframe);
}

#[test]
fn test_encoded_frame_methods() {
    let data = Bytes::from(vec![1u8, 2, 3]);
    let header = VideoFrameHeader {
        width: 640,
        height: 480,
        timestamp_us: 500,
        is_keyframe: true,
        codec: VideoCodec::VP9,
    };
    
    let frame = EncodedFrame::new(data, header.clone());
    
    // 测试 data() 方法
    assert_eq!(frame.data().len(), 3);
    
    // 测试 len() 方法
    assert_eq!(frame.len(), 3);
    
    // 测试 is_empty() 方法
    assert!(!frame.is_empty());
    
    // 测试空帧
    let empty_frame = EncodedFrame::new(Bytes::new(), header);
    assert!(empty_frame.is_empty());
    assert_eq!(empty_frame.len(), 0);
}

#[test]
fn test_bytes_clone_is_zero_copy() {
    // 验证 Bytes 克隆是零拷贝的
    let original = Bytes::from(vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let clone1 = original.clone();
    let clone2 = original.clone();
    
    // 所有克隆都应该指向相同的数据
    assert_eq!(original.len(), clone1.len());
    assert_eq!(original.len(), clone2.len());
    assert_eq!(&original[..], &clone1[..]);
    assert_eq!(&original[..], &clone2[..]);
}
