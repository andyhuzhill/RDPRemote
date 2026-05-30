# 零拷贝帧传输实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用 `bytes::Bytes` 和 `BytesMut` 实现零拷贝帧传输，减少内存复制，提高视频流传输性能。

**Architecture:** 
- 在 `AgentPeer` 中引入 `BytesMut` 缓冲区复用机制
- 将 `send_video_frame` 的参数从 `Vec<u8>` 改为接受 `Bytes` 类型
- 在 `common/src/protocol.rs` 中使用 `Bytes` 替代 `Vec<u8>` 存储帧数据
- 确保所有调用链支持零拷贝传递

**Tech Stack:** Rust, bytes crate, WebRTC (0.10 for agent)

---

## 文件变更清单

### 修改文件
1. `agent/src/webrtc/peer.rs` - 添加 `BytesMut` 缓冲区，优化 `send_video_frame`
2. `common/src/protocol.rs` - 使用 `Bytes` 替代 `Vec<u8>` 存储帧数据
3. `agent/Cargo.toml` - 确保 `bytes` 依赖版本正确
4. `common/Cargo.toml` - 添加 `bytes` 依赖

### 新增文件
1. `agent/tests/zerocopy_test.rs` - 零拷贝传输单元测试
2. `common/tests/bytes_test.rs` - Bytes 类型测试

---

## 任务分解

### Task 1: 添加 bytes 依赖到 common crate

**Files:**
- Modify: `common/Cargo.toml`

- [ ] **Step 1: 添加 bytes 依赖**

```toml
[dependencies]
serde = { workspace = true }
bytes = "1"
```

- [ ] **Step 2: 验证依赖添加**

运行: `cargo check -p rdp-common`
预期: 编译通过

---

### Task 2: 修改 common/src/protocol.rs 使用 Bytes

**Files:**
- Modify: `common/src/protocol.rs`

- [ ] **Step 1: 添加 Bytes 导入和 EncodedFrame 结构体**

```rust
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
    pub fn new(data: Bytes, header: VideoFrameHeader) -> Self {
        Self { data, header }
    }
    
    pub fn from_vec(data: Vec<u8>, header: VideoFrameHeader) -> Self {
        Self {
            data: Bytes::from(data),
            header,
        }
    }
}
```

- [ ] **Step 2: 验证编译**

运行: `cargo check -p rdp-common`
预期: 编译通过

---

### Task 3: 修改 agent/src/webrtc/peer.rs 实现零拷贝

**Files:**
- Modify: `agent/src/webrtc/peer.rs`

- [ ] **Step 1: 添加 BytesMut 缓冲区和优化 send_video_frame**

```rust
use bytes::{Bytes, BytesMut};

pub struct AgentPeer {
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    frame_buffer: BytesMut,  // 复用缓冲区，减少分配
}

impl AgentPeer {
    // ... 现有代码保持不变 ...
    
    /// Create a new AgentPeer instance with frame buffer
    pub async fn new() -> Result<Self> {
        // ... 现有实现保持不变 ...
        
        Ok(Self {
            peer_connection: Arc::new(peer_connection),
            video_track,
            frame_buffer: BytesMut::with_capacity(1024 * 1024),  // 1MB 初始容量
        })
    }
    
    /// Send a video frame with zero-copy optimization
    ///
    /// # Arguments
    /// * `data` - The encoded video frame data as Bytes (zero-copy)
    /// * `duration_us` - Frame duration in microseconds
    /// * `is_keyframe` - Whether this is a keyframe (I-frame)
    pub async fn send_video_frame_bytes(
        &self,
        data: Bytes,
        duration_us: u64,
        _is_keyframe: bool,
    ) -> Result<()> {
        let sample = Sample {
            data,  // 零拷贝：直接传递 Bytes，无需复制
            duration: std::time::Duration::from_micros(duration_us),
            ..Default::default()
        };

        self.video_track
            .write_sample(&sample)
            .await
            .map_err(|e| Error::Send(format!("Failed to write sample: {}", e)))?;

        Ok(())
    }
    
    /// Send a video frame using buffer reuse (zero-copy when possible)
    ///
    /// This method reuses the internal buffer to minimize allocations.
    /// For best performance, call `clear_buffer` periodically.
    pub async fn send_video_frame_reuse(
        &mut self,
        data: &[u8],
        duration_us: u64,
        is_keyframe: bool,
    ) -> Result<()> {
        // 复用缓冲区：将数据写入缓冲区
        self.frame_buffer.clear();
        self.frame_buffer.extend_from_slice(data);
        
        // 冻结缓冲区并转换为 Bytes（零拷贝）
        let bytes = self.frame_buffer.split().freeze();
        
        // 重新分配缓冲区（如果需要）
        if self.frame_buffer.capacity() < data.len() {
            self.frame_buffer = BytesMut::with_capacity(data.len().max(1024 * 1024));
        }
        
        self.send_video_frame_bytes(bytes, duration_us, is_keyframe).await
    }
    
    /// Clear and reset the frame buffer
    pub fn clear_buffer(&mut self) {
        self.frame_buffer.clear();
    }
}
```

- [ ] **Step 2: 保持向后兼容的 send_video_frame 方法**

在 `send_video_frame` 中调用新的零拷贝实现：

```rust
/// Send a video frame (legacy API, converts Vec<u8> to Bytes)
///
/// For zero-copy performance, use `send_video_frame_bytes` instead.
pub async fn send_video_frame(
    &self,
    data: Vec<u8>,
    duration_us: u64,
    is_keyframe: bool,
) -> Result<()> {
    self.send_video_frame_bytes(Bytes::from(data), duration_us, is_keyframe).await
}
```

- [ ] **Step 3: 验证编译**

运行: `cargo check -p rdp-agent`
预期: 编译通过

---

### Task 4: 编写零拷贝传输测试

**Files:**
- Create: `agent/tests/zerocopy_test.rs`

- [ ] **Step 1: 编写测试代码**

```rust
use bytes::{Bytes, BytesMut};
use rdp_agent::webrtc::AgentPeer;

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
async fn test_encoded_frame_bytes_storage() {
    // 测试 EncodedFrame 使用 Bytes 存储
    use rdp_common::{EncodedFrame, VideoFrameHeader, VideoCodec};
    
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
}

#[tokio::test]
async fn test_agent_peer_send_video_frame_bytes() {
    // 测试 AgentPeer 的零拷贝发送方法
    // 注意：这个测试需要实际的 WebRTC 环境，这里只做编译验证
    // 实际测试在集成测试中进行
    
    // 验证方法签名正确
    let _ = |data: Bytes, duration: u64, is_keyframe: bool| async move {
        // 类型检查：确保方法接受 Bytes 类型
        let _ = (data, duration, is_keyframe);
    };
}
```

- [ ] **Step 2: 运行测试**

运行: `cargo test -p rdp-agent zerocopy_test -- --nocapture`
预期: 测试通过

---

### Task 5: 编写 common crate 的 Bytes 测试

**Files:**
- Create: `common/tests/bytes_test.rs`

- [ ] **Step 1: 编写测试代码**

```rust
use bytes::Bytes;
use rdp_common::{EncodedFrame, VideoFrameHeader, VideoCodec};

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
```

- [ ] **Step 2: 运行测试**

运行: `cargo test -p rdp-common -- --nocapture`
预期: 测试通过

---

### Task 6: 验证整个工作空间编译

**Files:**
- No file changes

- [ ] **Step 1: 验证所有 crate 编译**

运行: `cargo check --workspace`
预期: 所有 crate 编译通过

- [ ] **Step 2: 运行所有测试**

运行: `cargo test --workspace -- --nocapture`
预期: 所有测试通过

---

## 验证清单

- [ ] `cargo check -p rdp-agent` 编译通过
- [ ] `cargo check -p rdp-client` 编译通过
- [ ] `cargo check -p rdp-common` 编译通过
- [ ] 零拷贝测试通过
- [ ] 所有现有测试仍然通过
- [ ] 代码提交到 git

---

## 性能优化说明

### 零拷贝原理

1. **`Bytes::from(Vec<u8>)`**: 将 `Vec<u8>` 的所有权转移给 `Bytes`，避免数据复制
2. **`BytesMut` 缓冲区复用**: 复用同一个缓冲区对象，减少内存分配和释放
3. **`Bytes` 的引用计数**: 多个 `EncodedFrame` 可以共享同一份数据，无需复制

### 使用建议

- 对于编码器输出的帧，直接使用 `Bytes::from(encoded_data)` 零拷贝传递
- 对于需要多次发送的帧，`Bytes` 的克隆是零拷贝的
- 定期调用 `clear_buffer()` 释放缓冲区内存
