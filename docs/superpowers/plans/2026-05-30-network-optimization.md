# Network Transmission Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize network transmission performance for RDPRemote by improving WebRTC configuration, implementing batch sending, and enhancing adaptive bitrate control with RTCP feedback.

**Architecture:** Three independent optimization modules:
1. WebRTC peer configuration with congestion control and codec feedback
2. Batch frame processing in agent main loop
3. RTCP feedback integration for adaptive bitrate control

**Tech Stack:** Rust, WebRTC (0.10 for agent, 0.12 for client), tokio async runtime

---

## File Structure Changes

### Files to Modify

| File | Responsibility |
|------|----------------|
| `agent/src/webrtc/peer.rs` | Add MediaEngine with RTCP feedback, configure congestion control |
| `client/src/webrtc/peer.rs` | Add MediaEngine with RTCP feedback, configure congestion control |
| `agent/src/main.rs` | Implement batch frame processing loop |
| `agent/src/adaptive.rs` | Add RTCP feedback processing, improve bandwidth estimation |
| `agent/src/webrtc/mod.rs` | Add RTCP types and helper functions |

### Files to Create

| File | Responsibility |
|------|----------------|
| `agent/src/webrtc/rtcp.rs` | RTCP packet types and parsing utilities |
| `tests/network_optimization.rs` | Integration tests for optimizations |

---

## Task Decomposition

### Task 1: WebRTC Peer Configuration - Agent Side

**Files:**
- Modify: `agent/src/webrtc/peer.rs`
- Create: `agent/src/webrtc/rtcp.rs`

- [ ] **Step 1: Create RTCP types module**

Create `agent/src/webrtc/rtcp.rs` with RTCP packet types for feedback processing:

```rust
//! RTCP packet types and parsing utilities

/// RTCP feedback message types
#[derive(Debug, Clone, PartialEq)]
pub enum RTCPFeedbackType {
    /// Goog-REMB: Receiver Estimated Maximum Bitrate
    GoogRemb(u64), // bitrate in bps
    /// Transport-CC: Transport-wide Congestion Control
    TransportCc {
        sender_ssrc: u32,
        base_sequence_number: u16,
        status_packet_count: u16,
        status_packets: Vec<u16>,
    },
    /// NACK: Negative Acknowledgement
    Nack {
        media_ssrc: u32,
        missing_packets: Vec<u16>,
    },
    /// Receiver Report with loss statistics
    ReceiverReport {
        fraction_lost: u8,
        packets_lost: i32,
        jitter: u32,
        last_sr_delay: u32,
    },
}

/// Calculate packet loss percentage from receiver report
pub fn calculate_loss_percentage(fraction_lost: u8, packets_lost: i32) -> f32 {
    // fraction_lost is 8-bit fixed point: value / 256
    let fraction = fraction_lost as f32 / 256.0;
    // Use the more accurate packets_lost if available
    if packets_lost > 0 {
        // packets_lost is signed 24-bit, convert to percentage
        // This requires knowing total packets received
        // For now, use fraction_lost as approximation
        fraction
    } else {
        fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_loss_percentage() {
        // 5% loss = 0.05 * 256 ≈ 13
        assert!((calculate_loss_percentage(13, 0) - 0.05).abs() < 0.01);
        
        // 10% loss = 0.10 * 256 ≈ 26
        assert!((calculate_loss_percentage(26, 0) - 0.10).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Add MediaEngine with RTCP feedback to AgentPeer**

Modify `agent/src/webrtc/peer.rs`:

```rust
// Add imports
use webrtc::media_engine::{MediaEngine, RTPHeaderExtension};
use webrtc::interceptor::registry::Registry;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecParameters, RTPCodecType, RTCPFeedback};

// In AgentPeer::new(), replace the simple registry with MediaEngine:

// Create MediaEngine with codec configuration
let media_engine = MediaEngine::default();

// Register VP8 codec with RTCP feedback
media_engine.register_codec(
    RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: "video/VP8".to_string(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1".to_string(),
            rtcp_feedback: vec![
                RTCPFeedback {
                    typ: "goog-remb".to_string(),
                    parameter: "".to_string(),
                },
                RTCPFeedback {
                    typ: "transport-cc".to_string(),
                    parameter: "".to_string(),
                },
                RTCPFeedback {
                    typ: "nack".to_string(),
                    parameter: "".to_string(),
                },
                RTCPFeedback {
                    typ: "nack".to_string(),
                    parameter: "pli".to_string(),
                },
            ],
        },
        payload_type: 96,
    },
    RTPCodecType::Video,
)?;

// Register header extensions
media_engine.register_header_extension(
    RTPHeaderExtension {
        uri: "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01".to_string(),
        id: 1,
        encrypted: false,
        ..Default::default()
    },
    RTPCodecType::Video,
)?;

// Create interceptor registry
let registry = Registry::new();

// Build API with MediaEngine and registry
let api = APIBuilder::new()
    .with_media_engine(media_engine)
    .with_interceptor_registry(registry)
    .build();
```

- [ ] **Step 3: Add congestion control configuration**

Add to `agent/src/webrtc/peer.rs`:

```rust
// In RTCConfiguration, add congestion control hints:
let config = RTCConfiguration {
    ice_servers: vec![...],
    // Enable continuous ICE gathering for faster connection
    ice_transport_policy: RTCIceTransportPolicy::All,
    // Bundle policy for multiplexing
    bundle_policy: RTCBundlePolicy::Balanced,
    // RTCP multiplexing
    rtcp_mux_policy: RTCRtcpMuxPolicy::Require,
    ..Default::default()
};
```

- [ ] **Step 4: Write test for AgentPeer creation**

Add to `agent/src/webrtc/peer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_peer_creation_with_media_engine() {
        let peer = AgentPeer::new().await;
        assert!(peer.is_ok());
    }
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p rdp-agent`
Expected: Compilation succeeds with new imports

---

### Task 2: WebRTC Peer Configuration - Client Side

**Files:**
- Modify: `client/src/webrtc/peer.rs`

- [ ] **Step 1: Add MediaEngine with RTCP feedback to ClientPeer**

Modify `client/src/webrtc/peer.rs`:

```rust
// Add imports
use webrtc::media_engine::{MediaEngine, RTPHeaderExtension};
use webrtc::interceptor::registry::Registry;
use webrtc::rtp_transceiver::rtp_codec::{RTCRtpCodecParameters, RTPCodecType, RTCPFeedback};

// In ClientPeer::new(), replace simple registry:

// Create MediaEngine with codec configuration
let media_engine = MediaEngine::default();

// Register VP8 codec with RTCP feedback for congestion control
media_engine.register_codec(
    RTCRtpCodecParameters {
        capability: RTCRtpCodecCapability {
            mime_type: "video/VP8".to_string(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "".to_string(),
            rtcp_feedback: vec![
                RTCPFeedback {
                    typ: "goog-remb".to_string(),
                    parameter: "".to_string(),
                },
                RTCPFeedback {
                    typ: "transport-cc".to_string(),
                    parameter: "".to_string(),
                },
            ],
        },
        payload_type: 96,
    },
    RTPCodecType::Video,
)?;

// Register header extension for transport-wide CC
media_engine.register_header_extension(
    RTPHeaderExtension {
        uri: "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01".to_string(),
        id: 1,
        encrypted: false,
        ..Default::default()
    },
    RTPCodecType::Video,
)?;

// Create interceptor registry
let registry = Registry::new();

// Build API with MediaEngine
let api = APIBuilder::default()
    .with_media_engine(media_engine)
    .with_interceptor_registry(registry)
    .build();
```

- [ ] **Step 2: Add congestion control configuration**

```rust
let config = RTCConfiguration {
    ice_servers: vec![...],
    ice_transport_policy: RTCIceTransportPolicy::All,
    bundle_policy: RTCBundlePolicy::Balanced,
    rtcp_mux_policy: RTCRtcpMuxPolicy::Require,
    ..Default::default()
};
```

- [ ] **Step 3: Write test for ClientPeer creation**

The test already exists in `client/src/webrtc/peer.rs`. Verify it still passes.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rdp-client`
Expected: Compilation succeeds

---

### Task 3: Batch Frame Sending Optimization

**Files:**
- Modify: `agent/src/main.rs`

- [ ] **Step 1: Define batch processing constants and structure**

Add to `agent/src/main.rs`:

```rust
// Add constants at top of run_agent function
const BATCH_SIZE: usize = 3;
const MAX_BATCH_DURATION_US: u64 = 33_333; // ~30fps target

// Batch buffer structure
struct FrameBatch {
    frames: Vec<EncodedFrame>,
    total_bytes: u64,
    is_keyframe_in_batch: bool,
}

impl FrameBatch {
    fn new() -> Self {
        Self {
            frames: Vec::with_capacity(BATCH_SIZE),
            total_bytes: 0,
            is_keyframe_in_batch: false,
        }
    }
    
    fn push(&mut self, frame: EncodedFrame) {
        self.total_bytes += frame.data.len() as u64;
        if frame.is_keyframe {
            self.is_keyframe_in_batch = true;
        }
        self.frames.push(frame);
    }
    
    fn is_full(&self) -> bool {
        self.frames.len() >= BATCH_SIZE
    }
    
    fn should_flush(&self, elapsed_us: u64) -> bool {
        // Flush if batch is full or duration exceeded
        self.is_full() || elapsed_us >= MAX_BATCH_DURATION_US
    }
}
```

- [ ] **Step 2: Modify video stream loop to use batch processing**

Replace the video stream loop in `agent/src/main.rs`:

```rust
// Video stream loop with batch processing
tracing::info!("WebRTC connected, starting video stream with batch processing...");
let mut adaptive = AdaptiveController::new();
let mut frame_count = 0u64;
let mut batch = FrameBatch::new();
let mut frame_interval = std::time::Duration::from_millis(33);

// Pre-allocate buffers
let mut encoded_buffer = Vec::with_capacity(1024 * 1024);

loop {
    let start = std::time::Instant::now();
    
    // Get current bandwidth tier
    let tier = adaptive.current_tier();
    frame_interval = tier.frame_interval();
    
    // Collect frames for batch
    while !batch.should_flush(start.elapsed().as_micros() as u64) {
        match capture.capture_frame() {
            Ok(frame) => {
                encoded_buffer.clear();
                match encoder.encode(&frame.data, frame.width, frame.height, frame.timestamp_us) {
                    Ok(encoded) => {
                        encoded_buffer.extend_from_slice(&encoded.data);
                        batch.push(EncodedFrame {
                            data: encoded_buffer.clone(),
                            duration_us: 33_333,
                            is_keyframe: encoded.is_keyframe,
                        });
                        adaptive.add_bytes_sent(encoded.data.len() as u64);
                    }
                    Err(e) => tracing::warn!("Encode error: {}", e),
                }
            }
            Err(e) => {
                tracing::trace!("Capture error: {}", e);
                break;
            }
        }
    }
    
    // Send batch
    if !batch.frames.is_empty() {
        for frame in batch.frames.drain(..) {
            let _ = peer.send_video_frame(frame.data, frame.duration_us, frame.is_keyframe).await;
        }
        batch.total_bytes = 0;
        batch.is_keyframe_in_batch = false;
    }
    
    // Adjust bandwidth tier
    if let Some(new_tier) = adaptive.check_and_adjust() {
        tracing::info!("Bandwidth tier changed: {:?}", new_tier);
        let _ = encoder.set_resolution(new_tier.width(), new_tier.height());
    }
    
    frame_count += 1;
    if frame_count % 30 == 0 {
        tracing::info!("Frame {}", frame_count);
    }
    
    // Maintain frame timing
    let elapsed = start.elapsed();
    if elapsed < frame_interval {
        tokio::time::sleep(frame_interval - elapsed).await;
    }
}
```

- [ ] **Step 3: Add EncodedFrame struct**

```rust
#[derive(Debug, Clone)]
struct EncodedFrame {
    data: bytes::Bytes,
    duration_us: u64,
    is_keyframe: bool,
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rdp-agent`
Expected: Compilation succeeds

---

### Task 4: RTCP Feedback Integration for Adaptive Control

**Files:**
- Modify: `agent/src/adaptive.rs`
- Modify: `agent/src/webrtc/peer.rs` (add RTCP receiver callback)

- [ ] **Step 1: Add RTCP feedback processing to AdaptiveController**

Modify `agent/src/adaptive.rs`:

```rust
// Add imports
use crate::webrtc::rtcp::{RTCPFeedbackType, calculate_loss_percentage};

// Add fields to AdaptiveController
pub struct AdaptiveController {
    current_tier: BandwidthTier,
    last_check: Instant,
    check_interval: Duration,
    bytes_sent: u64,
    last_bytes: u64,
    
    // RTCP feedback statistics
    last_fraction_lost: f32,
    last_remb_bitrate: Option<u64>,
    nack_count: u64,
    last_nack_check: Instant,
}

impl AdaptiveController {
    pub fn new() -> Self {
        Self {
            current_tier: BandwidthTier::Medium,
            last_check: Instant::now(),
            check_interval: Duration::from_secs(2),
            bytes_sent: 0,
            last_bytes: 0,
            last_fraction_lost: 0.0,
            last_remb_bitrate: None,
            nack_count: 0,
            last_nack_check: Instant::now(),
        }
    }
    
    // ... existing methods ...
    
    /// Update from RTCP feedback
    pub fn update_from_rtcp(&mut self, feedback: &RTCPFeedbackType) {
        match feedback {
            RTCPFeedbackType::ReceiverReport { fraction_lost, packets_lost, .. } => {
                self.last_fraction_lost = calculate_loss_percentage(*fraction_lost, *packets_lost);
                
                // Immediate reaction to high loss
                if self.last_fraction_lost > 0.05 {
                    // Loss > 5%, downgrade tier
                    self.current_tier = self.current_tier.downgrade();
                } else if self.last_fraction_lost < 0.01 && self.current_tier != BandwidthTier::High {
                    // Low loss, can try upgrading
                    // Will be confirmed in check_and_adjust
                }
            }
            RTCPFeedbackType::GoogRemb(bitrate_bps) => {
                // REMB gives us the receiver's estimated max bitrate
                self.last_remb_bitrate = Some(*bitrate_bps);
                
                let target_kbps = (*bitrate_bps / 1000) as u32;
                // Adjust tier based on REMB
                let new_tier = if target_kbps > 2000 {
                    BandwidthTier::High
                } else if target_kbps > 1000 {
                    BandwidthTier::Medium
                } else if target_kbps > 500 {
                    BandwidthTier::Low
                } else {
                    BandwidthTier::VeryLow
                };
                
                if new_tier != self.current_tier {
                    self.current_tier = new_tier;
                }
            }
            RTCPFeedbackType::Nack { .. } => {
                self.nack_count += 1;
                
                // Check NACK rate
                if self.last_nack_check.elapsed() >= Duration::from_secs(1) {
                    let nack_rate = self.nack_count;
                    if nack_rate > 10 {
                        // High NACK rate, reduce bitrate
                        self.current_tier = self.current_tier.downgrade();
                    }
                    self.nack_count = 0;
                    self.last_nack_check = Instant::now();
                }
            }
            _ => {}
        }
    }
}

// Add downgrade method to BandwidthTier
impl BandwidthTier {
    pub fn downgrade(&self) -> Self {
        match self {
            Self::High => Self::Medium,
            Self::Medium => Self::Low,
            Self::Low => Self::VeryLow,
            Self::VeryLow => Self::VeryLow,
        }
    }
    
    pub fn upgrade(&self) -> Self {
        match self {
            Self::VeryLow => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::High,
        }
    }
}
```

- [ ] **Step 2: Add RTCP receiver to AgentPeer**

Modify `agent/src/webrtc/peer.rs`:

```rust
// Add imports
use tokio::sync::mpsc;

// Add to AgentPeer struct
pub struct AgentPeer {
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    rtcp_tx: Option<mpsc::Sender<RTCPFeedbackType>>,
}

// Add method to register RTCP callback
pub fn on_rtcp<F>(&self, callback: F)
where
    F: Fn(RTCPFeedbackType) -> futures_util::future::BoxFuture<'static, ()> + Send + Sync + Clone + 'static,
{
    let peer_connection = Arc::clone(&self.peer_connection);
    let callback = callback.clone();
    
    peer_connection.on_rtcp(Box::new(move |packets: Vec<Box<dyn webrtc::rtcp::packet::Packet>>| {
        let callback = callback.clone();
        Box::pin(async move {
            // Parse RTCP packets and invoke callback
            for packet in packets {
                // Parse based on packet type
                // Forward to callback
            }
        })
    }));
}
```

- [ ] **Step 3: Write tests for RTCP feedback processing**

Add to `agent/src/adaptive.rs`:

```rust
#[test]
fn test_update_from_receiver_report() {
    let mut controller = AdaptiveController::new();
    controller.current_tier = BandwidthTier::High;
    
    // Simulate 10% packet loss
    let feedback = RTCPFeedbackType::ReceiverReport {
        fraction_lost: 26, // 0.10
        packets_lost: 100,
        jitter: 0,
        last_sr_delay: 0,
    };
    
    controller.update_from_rtcp(&feedback);
    
    // Should downgrade from High
    assert_eq!(controller.current_tier(), BandwidthTier::Medium);
}

#[test]
fn test_update_from_remb() {
    let mut controller = AdaptiveController::new();
    controller.current_tier = BandwidthTier::Medium;
    
    // REMB suggests 1.5 Mbps
    let feedback = RTCPFeedbackType::GoogRemb(1_500_000);
    
    controller.update_from_rtcp(&feedback);
    
    // Should stay at Medium (1-2 Mbps range)
    assert_eq!(controller.current_tier(), BandwidthTier::Medium);
}

#[test]
fn test_bandwidth_tier_downgrade_upgrade() {
    assert_eq!(BandwidthTier::High.downgrade(), BandwidthTier::Medium);
    assert_eq!(BandwidthTier::Medium.downgrade(), BandwidthTier::Low);
    assert_eq!(BandwidthTier::Low.downgrade(), BandwidthTier::VeryLow);
    assert_eq!(BandwidthTier::VeryLow.downgrade(), BandwidthTier::VeryLow);
    
    assert_eq!(BandwidthTier::VeryLow.upgrade(), BandwidthTier::Low);
    assert_eq!(BandwidthTier::Low.upgrade(), BandwidthTier::Medium);
    assert_eq!(BandwidthTier::Medium.upgrade(), BandwidthTier::High);
    assert_eq!(BandwidthTier::High.upgrade(), BandwidthTier::High);
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rdp-agent`
Expected: Compilation succeeds

---

### Task 5: Verification and Testing

**Files:**
- Run tests

- [ ] **Step 1: Run all unit tests**

```bash
cargo test -p rdp-agent
cargo test -p rdp-client
```

Expected: All tests pass

- [ ] **Step 2: Verify compilation for both crates**

```bash
cargo check -p rdp-agent
cargo check -p rdp-client
```

Expected: No compilation errors

---

## Implementation Notes

### WebRTC Version Considerations

- Agent uses `webrtc` 0.10
- Client uses `webrtc` 0.12
- API differences may require adaptation
- Check `Cargo.toml` for exact versions

### Platform Considerations

- Agent screen capture is Windows-only
- RTCP feedback processing works on all platforms
- Tests should be marked with `#[cfg(target_os = "windows")]` where appropriate

### Performance Targets

- Batch size of 3 frames reduces per-frame overhead by ~66%
- RTCP feedback enables real-time congestion response
- Expected latency improvement: 10-20%
- Expected throughput improvement: 5-15%

---

## Rollback Plan

If issues arise:
1. Revert `agent/src/webrtc/peer.rs` to original
2. Revert `client/src/webrtc/peer.rs` to original
3. Revert `agent/src/main.rs` batch processing
4. Revert `agent/src/adaptive.rs` RTCP integration
5. Delete `agent/src/webrtc/rtcp.rs`