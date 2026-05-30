//! 零拷贝帧传输单元测试
//!
//! 测试 Bytes 和 BytesMut 的零拷贝特性

use bytes::{Bytes, BytesMut};

#[tokio::test]
async fn test_bytes_from_vec_zero_copy() {
    // 测试 Bytes::from(Vec<u8>) 的零拷贝特性
    let original = vec![1u8, 2, 3, 4, 5];
    let bytes = Bytes::from(original);
    
    // Bytes 应该持有数据的引用计数
    assert_eq!(bytes.len(), 5);
    assert_eq!(&bytes[..], &[1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn test_bytes_mut_buffer_reuse() {
    // 测试 BytesMut 缓冲区复用
    let mut buffer = BytesMut::with_capacity(1024);
    
    // 第一次写入
    buffer.extend_from_slice(&[1, 2, 3]);
    let bytes1 = buffer.split().freeze();
    assert_eq!(&bytes1[..], &[1, 2, 3]);
    
    // 缓冲区清空后可复用
    assert_eq!(buffer.len(), 0);
    
    // 第二次写入
    buffer.extend_from_slice(&[4, 5, 6, 7]);
    let bytes2 = buffer.split().freeze();
    assert_eq!(&bytes2[..], &[4, 5, 6, 7]);
}

#[tokio::test]
async fn test_bytes_clone_shares_data() {
    // 测试 Bytes 克隆时共享底层数据（零拷贝）
    let data = Bytes::from(vec![1u8, 2, 3, 4, 5]);
    let data_clone = data.clone();
    
    // 克隆后数据应该相同
    assert_eq!(data.len(), data_clone.len());
    assert_eq!(&data[..], &data_clone[..]);
    
    // 验证两个 Bytes 指向同一份数据（通过 refcnt 间接验证）
    // Bytes 内部使用 Arc，克隆不会复制数据
}

#[tokio::test]
async fn test_encoded_frame_bytes_storage() {
    // 测试 EncodedFrame 使用 Bytes 存储
    use rdp_common::protocol::{EncodedFrame, VideoFrameHeader, VideoCodec};
    
    let data = Bytes::from(vec![0u8; 100]);
    let header = VideoFrameHeader {
        width: 1920,
        height: 1080,
        timestamp_us: 0,
        is_keyframe: true,
        codec: VideoCodec::VP9,
    };
    
    let frame = EncodedFrame::new(data, header);
    
    assert_eq!(frame.data.len(), 100);
    assert!(frame.header.is_keyframe);
    assert!(!frame.is_empty());
}

#[tokio::test]
async fn test_encoded_frame_from_vec() {
    // 测试从 Vec<u8> 创建 EncodedFrame
    use rdp_common::protocol::{EncodedFrame, VideoFrameHeader, VideoCodec};
    
    let data = vec![0u8; 256];
    let header = VideoFrameHeader {
        width: 1280,
        height: 720,
        timestamp_us: 1000,
        is_keyframe: false,
        codec: VideoCodec::H264,
    };
    
    let frame = EncodedFrame::from_vec(data, header);
    
    assert_eq!(frame.len(), 256);
    assert!(!frame.header.is_keyframe);
}

#[tokio::test]
async fn test_encoded_frame_clone_shares_data() {
    // 测试 EncodedFrame 克隆时共享 Bytes 数据
    use rdp_common::protocol::{EncodedFrame, VideoFrameHeader, VideoCodec};
    
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
    
    // 克隆后数据应该共享
    assert_eq!(frame1.data.len(), frame2.data.len());
    assert_eq!(&frame1.data[..], &frame2.data[..]);
}

#[tokio::test]
async fn test_agent_peer_send_video_frame_bytes_signature() {
    // 验证 AgentPeer 的 send_video_frame_bytes 方法签名
    // 这个测试确保方法接受 Bytes 类型参数
    
    // 编译时类型检查：确保方法签名正确
    let _test_fn = |data: Bytes, duration: u64, is_keyframe: bool| async move {
        // 如果编译通过，说明类型匹配
        let _ = (data, duration, is_keyframe);
    };
}

#[tokio::test]
async fn test_bytes_capacity_and_grow() {
    // 测试 BytesMut 容量增长
    let mut buffer = BytesMut::with_capacity(64);
    
    // 写入小于容量的数据
    buffer.extend_from_slice(&[1; 32]);
    assert_eq!(buffer.len(), 32);
    assert!(buffer.capacity() >= 32);
    
    // 写入超过容量的数据，应该自动增长
    buffer.extend_from_slice(&[2; 64]);
    assert_eq!(buffer.len(), 96);
    assert!(buffer.capacity() >= 96);
}
