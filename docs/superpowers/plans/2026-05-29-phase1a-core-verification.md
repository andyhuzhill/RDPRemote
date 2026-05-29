# Phase 1a: 局域网核心验证 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在局域网内实现 屏幕捕获 → VP9 编码 → WebRTC 传输 → 解码渲染 的端到端视频流管道

**Architecture:** 
- 被控端 (Agent): Windows DXGI 屏幕捕获 + VP9 软件编码 + WebRTC 发送
- 控制端 (Client): WebRTC 接收 + VP9 解码 + wgpu 渲染
- 信令: 简单 WebSocket 服务器 (tokio-tungstenite)
- WebRTC 栈: webrtc-rs (纯 Rust，Phase 1a 快速验证)

**Tech Stack:** Rust, webrtc-rs, windows-rs (DXGI), wgpu, tokio, tokio-tungstenite

**Pragmatic Note:** 设计文档指定 libwebrtc C++ FFI，但 Phase 1a 使用 webrtc-rs 快速验证管道。
待管道验证通过后，Phase 1b 评估是否需要切换到 libwebrtc FFI 以获得更完整的功能支持。

---

## 文件结构

```
RDPRemote/
├── Cargo.toml                    # Workspace 根配置
├── agent/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Agent 入口
│       ├── screen/
│       │   ├── mod.rs            # 屏幕捕获 trait
│       │   └── dxgi.rs           # DXGI Desktop Duplication 实现
│       ├── encoder/
│       │   ├── mod.rs            # 编码器 trait
│       │   └── vp9.rs            # VP9 软件编码 (libvpx FFI)
│       └── webrtc/
│           ├── mod.rs            # WebRTC 发送端
│           └── peer.rs           # PeerConnection 管理
├── client/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Client 入口
│       ├── render/
│       │   ├── mod.rs            # 渲染器 trait
│       │   └── wgpu_renderer.rs  # wgpu 视频帧渲染
│       └── webrtc/
│           ├── mod.rs            # WebRTC 接收端
│           └── peer.rs           # PeerConnection 管理
├── common/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # 公共类型
│       ├── signaling.rs          # WebSocket 信令消息定义
│       └── protocol.rs           # 视频帧协议
├── server/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs               # 简单 WebSocket 信令服务器
└── docs/
    └── specs/
        └── 2026-05-29-low-bandwidth-remote-desktop-design.md
```

---

## Task 1: 项目脚手架

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `common/Cargo.toml`, `common/src/lib.rs`, `common/src/signaling.rs`, `common/src/protocol.rs`
- Create: `server/Cargo.toml`, `server/src/main.rs`
- Create: `agent/Cargo.toml`, `agent/src/main.rs`
- Create: `client/Cargo.toml`, `client/src/main.rs`

- [ ] **Step 1: 创建 workspace Cargo.toml**

```toml
[workspace]
members = [
    "common",
    "server",
    "agent",
    "client",
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

- [ ] **Step 2: 创建 common crate**

`common/Cargo.toml`:
```toml
[package]
name = "rdp-common"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
```

`common/src/lib.rs`:
```rust
pub mod signaling;
pub mod protocol;
```

`common/src/signaling.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    #[serde(rename = "offer")]
    Offer { sdp: String },
    #[serde(rename = "answer")]
    Answer { sdp: String },
    #[serde(rename = "ice-candidate")]
    IceCandidate { candidate: String, sdp_mid: String, sdp_m_line_index: u16 },
    #[serde(rename = "register")]
    Register { device_id: String },
    #[serde(rename = "connect")]
    Connect { target_device_id: String },
    #[serde(rename = "error")]
    Error { message: String },
}
```

`common/src/protocol.rs`:
```rust
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
```

- [ ] **Step 3: 创建 server crate**

`server/Cargo.toml`:
```toml
[package]
name = "rdp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
rdp-common = { path = "../common" }
tokio.workspace = true
tokio-tungstenite = "0.26"
futures-util = "0.3"
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dashmap = "6"
```

`server/src/main.rs`:
```rust
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use rdp_common::signaling::SignalingMessage;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tracing::{info, warn, error};

type DeviceMap = Arc<DashMap<String, mpsc::UnboundedSender<String>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let devices: DeviceMap = Arc::new(DashMap::new());
    let addr = "0.0.0.0:8765";
    let listener = TcpListener::bind(addr).await?;
    info!("Signaling server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let devices = devices.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, peer_addr, devices).await {
                error!("Error handling {}: {}", peer_addr, e);
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    devices: DeviceMap,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    info!("New WebSocket connection from {}", peer_addr);
    
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    
    let mut device_id: Option<String> = None;
    
    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        match serde_json::from_str::<SignalingMessage>(&text) {
                            Ok(SignalingMessage::Register { id }) => {
                                device_id = Some(id.clone());
                                devices.insert(id.clone(), tx.clone());
                                info!("Device registered: {}", id);
                            }
                            Ok(SignalingMessage::Connect { target_device_id }) => {
                                if let Some(target_tx) = devices.get(&target_device_id) {
                                    let connect_msg = SignalingMessage::Connect {
                                        target_device_id: device_id.clone().unwrap_or_default(),
                                    };
                                    let _ = target_tx.send(serde_json::to_string(&connect_msg).unwrap());
                                }
                            }
                            Ok(msg) => {
                                // Forward to target device
                                if let Some(ref id) = device_id {
                                    // For simplicity, broadcast to all other devices
                                    for entry in devices.iter() {
                                        if entry.key() != id {
                                            let _ = entry.value().send(text.clone());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message from {}: {}", peer_addr, e);
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        error!("WebSocket error from {}: {}", peer_addr, e);
                        break;
                    }
                    None => break,
                }
            }
            Some(msg) = rx.recv() => {
                if ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        }
    }
    
    // Cleanup
    if let Some(id) = device_id {
        devices.remove(&id);
        info!("Device disconnected: {}", id);
    }
    
    Ok(())
}
```

- [ ] **Step 4: 创建 agent 和 client 的最小 main.rs**

`agent/Cargo.toml`:
```toml
[package]
name = "rdp-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
rdp-common = { path = "../common" }
tokio.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

`agent/src/main.rs`:
```rust
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("RDPRemote Agent starting");
    // TODO: Implement agent
    Ok(())
}
```

`client/Cargo.toml`:
```toml
[package]
name = "rdp-client"
version = "0.1.0"
edition = "2021"

[dependencies]
rdp-common = { path = "../common" }
tokio.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

`client/src/main.rs`:
```rust
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("RDPRemote Client starting");
    // TODO: Implement client
    Ok(())
}
```

- [ ] **Step 5: 验证项目编译**

Run: `cargo check`
Expected: 所有 crate 编译通过

- [ ] **Step 6: 提交**

```bash
git add -A
git commit -m "feat: project scaffolding with workspace structure"
```

---

## Task 2: 屏幕捕获 (DXGI Desktop Duplication)

**Files:**
- Create: `agent/src/screen/mod.rs`, `agent/src/screen/dxgi.rs`
- Modify: `agent/Cargo.toml`

- [ ] **Step 1: 添加 windows 依赖**

`agent/Cargo.toml` 添加:
```toml
[dependencies]
# ... existing deps ...
windows = { version = "0.58", features = [
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Dxgi_Common",
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_Direct3D",
    "Win32_Graphics_Direct3D_Fxc",
    "Win32_Graphics_Direct3D_Dxc",
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 2: 定义屏幕捕获 trait**

`agent/src/screen/mod.rs`:
```rust
pub mod dxgi;

pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub stride: u32,
    pub timestamp_us: u64,
}

pub trait ScreenCapture {
    fn capture_frame(&mut self) -> anyhow::Result<CapturedFrame>;
    fn get_dimensions(&self) -> (u32, u32);
}
```

- [ ] **Step 3: 实现 DXGI Desktop Duplication**

`agent/src/screen/dxgi.rs`:
```rust
use super::{CapturedFrame, ScreenCapture};
use anyhow::{anyhow, Result};
use windows::Win32::Graphics::{
    Dxgi::{IDXGIFactory1, IDXGIOutputDuplication, DXGI_OUTDUPL_FRAME_INFO},
    Direct3D11::{
        ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
        D3D11_CPU_ACCESS_READ, D3D11_USAGE_STAGING,
    },
    Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
};
use std::time::Instant;

pub struct DxDuplication {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: ID3D11Texture2D,
    width: u32,
    height: u32,
    start_time: Instant,
}

impl DxDuplication {
    pub fn new() -> Result<Self> {
        unsafe {
            let factory: IDXGIFactory1 = windows::Win32::Graphics::Dxgi::CreateDXGIFactory1()?;
            
            let adapter = factory.EnumAdapters1(0)?;
            
            let mut device = None;
            let mut context = None;
            
            let hr = windows::Win32::Graphics::Direct3D11::D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D11_SDK_VERSION]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            );
            
            let device = device.ok_or_else(|| anyhow!("Failed to create D3D11 device"))?;
            let context = context.ok_or_else(|| anyhow!("Failed to create D3D11 context"))?;
            
            let output = adapter.EnumOutputs(0)?;
            let output1 = output.cast::<windows::Win32::Graphics::Dxgi::IDXGIOutput1>()?;
            
            let duplication = output1.DuplicateOutput(&device)?;
            
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            desc.Width = 1920; // Will be updated on first capture
            desc.Height = 1080;
            desc.MipLevels = 1;
            desc.ArraySize = 1;
            desc.Format = windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
            desc.SampleDesc.Count = 1;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
            
            let mut staging_texture = None;
            device.CreateTexture2D(&desc, None, Some(&mut staging_texture))?;
            let staging_texture = staging_texture.ok_or_else(|| anyhow!("Failed to create staging texture"))?;
            
            Ok(Self {
                device,
                context,
                duplication,
                staging_texture,
                width: 1920,
                height: 1080,
                start_time: Instant::now(),
            })
        }
    }
}

impl ScreenCapture for DxDuplication {
    fn capture_frame(&mut self) -> Result<CapturedFrame> {
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            
            let hr = self.duplication.AcquireNextFrame(16, &mut frame_info, &mut resource);
            
            if hr.is_err() {
                return Err(anyhow!("Failed to acquire frame"));
            }
            
            let texture: ID3D11Texture2D = resource
                .ok_or_else(|| anyhow!("No frame resource"))?
                .cast()?;
            
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            
            if desc.Width != self.width || desc.Height != self.height {
                self.width = desc.Width;
                self.height = desc.Height;
                // Recreate staging texture
                desc.Usage = D3D11_USAGE_STAGING;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
                desc.BindFlags = windows::Win32::Graphics::Direct3D11::D3D11_BIND_FLAG(0);
                let mut new_staging = None;
                self.device.CreateTexture2D(&desc, None, Some(&mut new_staging))?;
                self.staging_texture = new_staging.ok_or_else(|| anyhow!("Failed to recreate staging texture"))?;
            }
            
            self.context.CopyResource(&self.staging_texture, &texture);
            
            let mut mapped = windows::Win32::Graphics::Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(
                &self.staging_texture,
                0,
                windows::Win32::Graphics::Direct3D11::D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;
            
            let stride = mapped.RowPitch;
            let data_size = (stride * self.height) as usize;
            let data = std::slice::from_raw_parts(mapped.pData as *const u8, data_size).to_vec();
            
            self.context.Unmap(&self.staging_texture, 0);
            self.duplication.ReleaseFrame()?;
            
            let timestamp_us = self.start_time.elapsed().as_micros() as u64;
            
            Ok(CapturedFrame {
                width: self.width,
                height: self.height,
                data,
                stride,
                timestamp_us,
            })
        }
    }
    
    fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p rdp-agent`
Expected: 编译通过（可能需要 Windows SDK）

- [ ] **Step 5: 提交**

```bash
git add agent/
git commit -m "feat: add DXGI screen capture skeleton"
```

---

## Task 3: VP9 软件编码 (libvpx FFI)

**Files:**
- Create: `agent/src/encoder/mod.rs`, `agent/src/encoder/vp9.rs`
- Modify: `agent/Cargo.toml`

- [ ] **Step 1: 添加 libvpx 依赖**

`agent/Cargo.toml` 添加:
```toml
[dependencies]
# ... existing deps ...
vpx-sys = "0.1"  # libvpx FFI bindings
```

- [ ] **Step 2: 定义编码器 trait**

`agent/src/encoder/mod.rs`:
```rust
pub mod vp9;

pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    pub timestamp_us: u64,
    pub width: u32,
    pub height: u32,
}

pub trait VideoEncoder {
    fn encode(&mut self, frame: &[u8], width: u32, height: u32, timestamp_us: u64) -> Result<EncodedFrame>;
    fn set_bitrate(&mut self, bitrate_kbps: u32);
    fn force_keyframe(&mut self);
}
```

- [ ] **Step 3: 实现 VP9 软件编码器**

`agent/src/encoder/vp9.rs`:
```rust
use super::{EncodedFrame, VideoEncoder};
use anyhow::{anyhow, Result};
use std::ptr;

pub struct Vp9Encoder {
    ctx: *mut vpx_sys::vpx_codec_ctx_t,
    cfg: vpx_sys::vpx_codec_enc_cfg_t,
    width: u32,
    height: u32,
    bitrate_kbps: u32,
    force_keyframe: bool,
}

unsafe impl Send for Vp9Encoder {}

impl Vp9Encoder {
    pub fn new(width: u32, height: u32, bitrate_kbps: u32) -> Result<Self> {
        unsafe {
            let iface = vpx_sys::vpx_codec_vp9_cx();
            
            let mut cfg = std::mem::zeroed::<vpx_sys::vpx_codec_enc_cfg_t>();
            let res = vpx_sys::vpx_codec_enc_config_default(iface, &mut cfg, 0);
            if res != vpx_sys::VPX_CODEC_OK {
                return Err(anyhow!("Failed to get default VP9 config"));
            }
            
            cfg.g_w = width;
            cfg.g_h = height;
            cfg.g_timebase.num = 1;
            cfg.g_timebase.den = 1_000_000; // microseconds
            cfg.rc_target_bitrate = bitrate_kbps;
            cfg.g_error_resilient = vpx_sys::VPX_ERROR_RESILIENT_DEFAULTS;
            cfg.g_pass = vpx_sys::VPX_RC_ONE_PASS;
            cfg.g_lag_in_frames = 0; // Zero latency
            cfg.rc_end_usage = vpx_sys::VPX_CBR;
            
            let mut ctx = Box::new(std::mem::zeroed::<vpx_sys::vpx_codec_ctx_t>());
            let ctx_ptr = Box::into_raw(ctx);
            
            let res = vpx_sys::vpx_codec_enc_init(
                ctx_ptr,
                iface,
                &cfg,
                vpx_sys::VPX_CODEC_USE_OUTPUT_PARTITION,
            );
            
            if res != vpx_sys::VPX_CODEC_OK {
                let _ = Box::from_raw(ctx_ptr);
                return Err(anyhow!("Failed to initialize VP9 encoder"));
            }
            
            // Set realtime speed
            vpx_sys::vpx_codec_control_(
                ctx_ptr,
                vpx_sys::VP8E_SET_CPUUSED as i32,
                8, // realtime preset
            );
            
            Ok(Self {
                ctx: ctx_ptr,
                cfg,
                width,
                height,
                bitrate_kbps,
                force_keyframe: false,
            })
        }
    }
}

impl VideoEncoder for Vp9Encoder {
    fn encode(&mut self, frame: &[u8], width: u32, height: u32, timestamp_us: u64) -> Result<EncodedFrame> {
        unsafe {
            if width != self.width || height != self.height {
                // Reinitialize encoder for new dimensions
                self.width = width;
                self.height = height;
                self.cfg.g_w = width;
                self.cfg.g_h = height;
                // Reinit would be needed here
            }
            
            let flags = if self.force_keyframe {
                self.force_keyframe = false;
                vpx_sys::VPX_EFLAG_FORCE_KF
            } else {
                0
            };
            
            // Convert BGRA to I420
            let i420_size = (width * height * 3 / 2) as usize;
            let mut i420 = vec![0u8; i420_size];
            bgra_to_i420(frame, &mut i420, width, height);
            
            let mut img = std::mem::zeroed::<vpx_sys::vpx_image_t>();
            vpx_sys::vpx_img_wrap(
                &mut img,
                vpx_sys::VPX_IMG_FMT_I420,
                width,
                height,
                1,
                i420.as_mut_ptr(),
            );
            
            let res = vpx_sys::vpx_codec_encode(
                self.ctx,
                &img,
                timestamp_us,
                1, // duration
                flags,
                vpx_sys::VPX_DL_REALTIME,
            );
            
            if res != vpx_sys::VPX_CODEC_OK {
                return Err(anyhow!("VP9 encode failed"));
            }
            
            let mut iter = ptr::null_mut();
            let mut encoded_data = Vec::new();
            let mut is_keyframe = false;
            
            loop {
                let pkt = vpx_sys::vpx_codec_get_cx_data(self.ctx, &mut iter);
                if pkt.is_null() {
                    break;
                }
                
                match (*pkt).kind {
                    vpx_sys::VPX_CODEC_CX_FRAME_PKT => {
                        let frame_data = (*pkt).data.frame;
                        let data = std::slice::from_raw_parts(
                            frame_data.buf as *const u8,
                            frame_data.sz as usize,
                        );
                        encoded_data.extend_from_slice(data);
                        is_keyframe = (frame_data.flags & vpx_sys::VPX_FRAME_IS_KEY) != 0;
                    }
                    _ => {}
                }
            }
            
            Ok(EncodedFrame {
                data: encoded_data,
                is_keyframe,
                timestamp_us,
                width: self.width,
                height: self.height,
            })
        }
    }
    
    fn set_bitrate(&mut self, bitrate_kbps: u32) {
        self.bitrate_kbps = bitrate_kbps;
        self.cfg.rc_target_bitrate = bitrate_kbps;
        unsafe {
            vpx_sys::vpx_codec_enc_config_set(self.ctx, &self.cfg);
        }
    }
    
    fn force_keyframe(&mut self) {
        self.force_keyframe = true;
    }
}

impl Drop for Vp9Encoder {
    fn drop(&mut self) {
        unsafe {
            if !self.ctx.is_null() {
                vpx_sys::vpx_codec_destroy(self.ctx);
                let _ = Box::from_raw(self.ctx);
            }
        }
    }
}

fn bgra_to_i420(bgra: &[u8], i420: &mut [u8], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);
    
    let y_plane = &mut i420[..y_size];
    let u_plane = &mut i420[y_size..y_size + uv_size];
    let v_plane = &mut i420[y_size + uv_size..];
    
    for row in 0..h {
        for col in 0..w {
            let bgra_idx = (row * w + col) * 4;
            let b = bgra[bgra_idx] as f32;
            let g = bgra[bgra_idx + 1] as f32;
            let r = bgra[bgra_idx + 2] as f32;
            
            let y = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            y_plane[row * w + col] = y;
            
            if row % 2 == 0 && col % 2 == 0 {
                let u = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0) as u8;
                let v = (0.500 * r - 0.419 * g - 0.081 * b + 128.0) as u8;
                u_plane[(row / 2) * (w / 2) + col / 2] = u;
                v_plane[(row / 2) * (w / 2) + col / 2] = v;
            }
        }
    }
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p rdp-agent`
Expected: 编译通过

- [ ] **Step 5: 提交**

```bash
git add agent/
git commit -m "feat: add VP9 software encoder skeleton"
```

---

## Task 4: WebRTC 发送端 (Agent)

**Files:**
- Create: `agent/src/webrtc/mod.rs`, `agent/src/webrtc/peer.rs`
- Modify: `agent/Cargo.toml`

- [ ] **Step 1: 添加 webrtc-rs 依赖**

`agent/Cargo.toml` 添加:
```toml
[dependencies]
# ... existing deps ...
webrtc = "0.12"
bytes = "1"
```

- [ ] **Step 2: 实现 WebRTC 发送端**

`agent/src/webrtc/mod.rs`:
```rust
pub mod peer;

pub use peer::AgentPeer;
```

`agent/src/webrtc/peer.rs`:
```rust
use anyhow::Result;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

pub struct AgentPeer {
    peer_connection: Arc<webrtc::peer_connection::RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
}

impl AgentPeer {
    pub async fn new() -> Result<Self> {
        let api = APIBuilder::new().build();
        
        let config = RTCConfiguration {
            ice_servers: vec![],
            ..Default::default()
        };
        
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP9".to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            "video".to_owned(),
            "rdpr-emote".to_owned(),
        ));
        
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn webrtc::track::track_local::TrackLocal + Send + Sync>)
            .await?;
        
        Ok(Self {
            peer_connection,
            video_track,
        })
    }
    
    pub async fn create_offer(&self) -> Result<String> {
        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection.set_local_description(offer.clone()).await?;
        Ok(offer.sdp)
    }
    
    pub async fn set_answer(&self, sdp: String) -> Result<()> {
        let answer = RTCSessionDescription::answer(sdp)?;
        self.peer_connection.set_remote_description(answer).await?;
        Ok(())
    }
    
    pub async fn add_ice_candidate(&self, candidate: String, sdp_mid: String, sdp_m_line_index: u16) -> Result<()> {
        self.peer_connection.add_ice_candidate(RTCIceCandidateInit {
            candidate,
            sdp_mid: Some(sdp_mid),
            sdp_m_line_index: Some(sdp_m_line_index),
            ..Default::default()
        }).await?;
        Ok(())
    }
    
    pub async fn send_video_frame(&self, data: Vec<u8>, duration_us: u64, is_keyframe: bool) -> Result<()> {
        let sample = Sample {
            data: Bytes::from(data),
            duration: std::time::Duration::from_micros(duration_us),
            ..Default::default()
        };
        
        self.video_track.write_sample(&sample).await?;
        Ok(())
    }
    
    pub fn on_ice_candidate<F>(&self, callback: F)
    where
        F: Fn(String, String, u16) + Send + Sync + 'static,
    {
        let pc = self.peer_connection.clone();
        tokio::spawn(async move {
            // Ice candidate handling would go here
            // For now, candidates are gathered via SDP
        });
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p rdp-agent`
Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add agent/
git commit -m "feat: add WebRTC sending peer for agent"
```

---

## Task 5: WebRTC 接收端 (Client)

**Files:**
- Create: `client/src/webrtc/mod.rs`, `client/src/webrtc/peer.rs`
- Modify: `client/Cargo.toml`

- [ ] **Step 1: 添加 webrtc-rs 依赖**

`client/Cargo.toml` 添加:
```toml
[dependencies]
# ... existing deps ...
webrtc = "0.12"
bytes = "1"
tokio.workspace = true
```

- [ ] **Step 2: 实现 WebRTC 接收端**

`client/src/webrtc/mod.rs`:
```rust
pub mod peer;

pub use peer::ClientPeer;
```

`client/src/webrtc/peer.rs`:
```rust
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

pub struct ReceivedVideoFrame {
    pub data: Vec<u8>,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
    pub width: u32,
    pub height: u32,
}

pub struct ClientPeer {
    peer_connection: Arc<webrtc::peer_connection::RTCPeerConnection>,
    frame_rx: mpsc::UnboundedReceiver<ReceivedVideoFrame>,
}

impl ClientPeer {
    pub async fn new() -> Result<(Self, mpsc::UnboundedReceiver<ReceivedVideoFrame>)> {
        let api = APIBuilder::new().build();
        
        let config = RTCConfiguration {
            ice_servers: vec![],
            ..Default::default()
        };
        
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        let (frame_tx, frame_rx) = mpsc::unbounded_channel();
        
        // Handle incoming tracks
        let pc = peer_connection.clone();
        peer_connection.on_track(Box::new(move |track, _, _| {
            let frame_tx = frame_tx.clone();
            Box::pin(async move {
                let codec = track.kind();
                tracing::info!("Received track: {:?}", codec);
                
                // Read RTP packets and decode
                let mut buf = vec![0u8; 1500];
                loop {
                    match track.read(&mut buf).await {
                        Ok((rtp_packet, _)) => {
                            // For now, just pass through the RTP payload
                            // In a real implementation, we'd decode VP9 here
                            let frame = ReceivedVideoFrame {
                                data: rtp_packet.payload.to_vec(),
                                timestamp_us: 0, // TODO: extract from RTP
                                is_keyframe: false, // TODO: detect keyframes
                                width: 0, // TODO: extract from SDP/headers
                                height: 0,
                            };
                            let _ = frame_tx.send(frame);
                        }
                        Err(e) => {
                            tracing::error!("Error reading track: {}", e);
                            break;
                        }
                    }
                }
            })
        }));
        
        Ok((Self { peer_connection }, frame_rx))
    }
    
    pub async fn create_answer(&self) -> Result<String> {
        let answer = self.peer_connection.create_answer(None).await?;
        self.peer_connection.set_local_description(answer.clone()).await?;
        Ok(answer.sdp)
    }
    
    pub async fn set_offer(&self, sdp: String) -> Result<()> {
        let offer = RTCSessionDescription::offer(sdp)?;
        self.peer_connection.set_remote_description(offer).await?;
        Ok(())
    }
    
    pub async fn add_ice_candidate(&self, candidate: String, sdp_mid: String, sdp_m_line_index: u16) -> Result<()> {
        self.peer_connection.add_ice_candidate(RTCIceCandidateInit {
            candidate,
            sdp_mid: Some(sdp_mid),
            sdp_m_line_index: Some(sdp_m_line_index),
            ..Default::default()
        }).await?;
        Ok(())
    }
    
    pub fn on_ice_candidate<F>(&self, callback: F)
    where
        F: Fn(String, String, u16) + Send + Sync + 'static,
    {
        let pc = self.peer_connection.clone();
        tokio::spawn(async move {
            // Ice candidate handling would go here
        });
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p rdp-client`
Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add client/
git commit -m "feat: add WebRTC receiving peer for client"
```

---

## Task 6: 视频渲染 (wgpu)

**Files:**
- Create: `client/src/render/mod.rs`, `client/src/render/wgpu_renderer.rs`
- Modify: `client/Cargo.toml`

- [ ] **Step 1: 添加 wgpu 和窗口依赖**

`client/Cargo.toml` 添加:
```toml
[dependencies]
# ... existing deps ...
wgpu = "24"
winit = "0.30"
pollster = "0.4"
```

- [ ] **Step 2: 定义渲染器 trait**

`client/src/render/mod.rs`:
```rust
pub mod wgpu_renderer;

pub trait VideoRenderer {
    fn render_frame(&mut self, data: &[u8], width: u32, height: u32);
    fn resize(&mut self, width: u32, height: u32);
}
```

- [ ] **Step 3: 实现 wgpu 渲染器**

`client/src/render/wgpu_renderer.rs`:
```rust
use super::VideoRenderer;
use wgpu::util::DeviceExt;
use winit::window::Window;

pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
    Vertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
    Vertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
    Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
];

const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

impl WgpuRenderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let surface = instance.create_surface(window.clone()).unwrap();
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Video Renderer Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .unwrap();
        
        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &surface_config);
        
        // Create texture for video frames
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Frame Texture"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Video Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Video Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        
        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Video Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Video Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        Self {
            device,
            queue,
            surface,
            surface_config,
            texture,
            bind_group,
            render_pipeline,
            vertex_buffer,
            index_buffer,
        }
    }
}

impl VideoRenderer for WgpuRenderer {
    fn render_frame(&mut self, data: &[u8], width: u32, height: u32) {
        // Update texture with new frame data
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Video Render Encoder"),
        });
        
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Video Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
        
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
    
    fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }
}
```

- [ ] **Step 4: 创建 shader**

`client/src/render/shader.wgsl`:
```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);
    out.tex_coords = model.tex_coords;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
```

- [ ] **Step 5: 验证编译**

Run: `cargo check -p rdp-client`
Expected: 编译通过

- [ ] **Step 6: 提交**

```bash
git add client/
git commit -m "feat: add wgpu video renderer"
```

---

## Task 7: 端到端集成

**Files:**
- Modify: `agent/src/main.rs`
- Modify: `client/src/main.rs`

- [ ] **Step 1: 实现 Agent 主循环**

`agent/src/main.rs`:
```rust
mod screen;
mod encoder;
mod webrtc;

use screen::ScreenCapture;
use encoder::VideoEncoder;
use webrtc::AgentPeer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("RDPRemote Agent starting");
    
    // Initialize screen capture
    let mut capture = screen::dxgi::DxDuplication::new()?;
    let (width, height) = capture.get_dimensions();
    info!("Screen capture initialized: {}x{}", width, height);
    
    // Initialize encoder
    let mut encoder = encoder::vp9::Vp9Encoder::new(width, height, 800)?;
    info!("VP9 encoder initialized: {}x{} @ 800kbps", width, height);
    
    // Initialize WebRTC
    let peer = AgentPeer::new().await?;
    info!("WebRTC peer created");
    
    // Create offer
    let offer = peer.create_offer().await?;
    info!("Offer created, waiting for answer...");
    
    // In Phase 1a, we'll print the offer SDP and wait for manual input
    println!("=== OFFER SDP ===");
    println!("{}", offer);
    println!("=================");
    println!("Paste the answer SDP:");
    
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    peer.set_answer(answer.trim().to_string()).await?;
    
    info!("WebRTC connected, starting capture loop");
    
    // Capture and send loop
    let frame_duration_us = 1_000_000 / 15; // 15fps
    let mut last_frame_time = std::time::Instant::now();
    
    loop {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(last_frame_time);
        
        if elapsed.as_micros() >= frame_duration_us {
            match capture.capture_frame() {
                Ok(frame) => {
                    match encoder.encode(&frame.data, frame.width, frame.height, frame.timestamp_us) {
                        Ok(encoded) => {
                            let _ = peer.send_video_frame(
                                encoded.data,
                                frame_duration_us as u64,
                                encoded.is_keyframe,
                            ).await;
                        }
                        Err(e) => {
                            tracing::warn!("Encode error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    // DXGI may fail if screen is locked or no changes
                    tracing::trace!("Capture error: {}", e);
                }
            }
            last_frame_time = now;
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}
```

- [ ] **Step 2: 实现 Client 主循环**

`client/src/main.rs`:
```rust
mod render;
mod webrtc;

use render::VideoRenderer;
use webrtc::ClientPeer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("RDPRemote Client starting");
    
    // Create window
    let event_loop = winit::event_loop::EventLoop::new()?;
    let window = Arc::new(winit::window::WindowBuilder::new()
        .with_title("RDPRemote")
        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
        .build(&event_loop)?);
    
    // Initialize renderer
    let mut renderer = render::wgpu_renderer::WgpuRenderer::new(window.clone()).await;
    info!("Renderer initialized");
    
    // Initialize WebRTC
    let (peer, mut frame_rx) = ClientPeer::new().await?;
    info!("WebRTC peer created");
    
    // In Phase 1a, we'll receive the offer SDP manually
    println!("Paste the offer SDP:");
    let mut offer = String::new();
    std::io::stdin().read_line(&mut offer)?;
    peer.set_offer(offer.trim().to_string()).await?;
    
    let answer = peer.create_answer().await?;
    println!("=== ANSWER SDP ===");
    println!("{}", answer);
    println!("==================");
    
    info!("WebRTC connected, waiting for frames...");
    
    // Spawn frame receiver
    let window_clone = window.clone();
    tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            // Render frame
            // renderer.render_frame(&frame.data, frame.width, frame.height);
            // For now, just log
            info!("Received frame: {} bytes", frame.data.len());
        }
    });
    
    // Run event loop
    event_loop.run(move |event, target| {
        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => {
                    target.exit();
                }
                winit::event::WindowEvent::Resized(new_size) => {
                    renderer.resize(new_size.width, new_size.height);
                }
                _ => {}
            },
            _ => {}
        }
    })?;
    
    Ok(())
}
```

- [ ] **Step 3: 验证完整编译**

Run: `cargo build`
Expected: 所有 crate 编译通过

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat: end-to-end integration for Phase 1a"
```

---

## 验证清单

完成所有任务后，验证以下内容：

- [ ] `cargo build` 编译成功
- [ ] `cargo check` 无警告
- [ ] 信令服务器可以启动: `cargo run -p rdp-server`
- [ ] Agent 可以启动并捕获屏幕: `cargo run -p rdp-agent` (需要 Windows + GPU)
- [ ] Client 可以启动: `cargo run -p rdp-client`
- [ ] 手动 SDP 交换后，视频流可以传输

---

## 后续 Phase

**Phase 1b: 网络穿透 + 自适应**
- STUN/TURN (coturn) 集成
- WebRTC GCC 自适应码率
- 自动 SDP 交换 (WebSocket 信令)

**Phase 2: 低带宽优化**
- 区域裁剪 (ROI + tile hash)
- 帧跳过策略
- 硬件编码 (NVENC/AMF/QSV)

**Phase 3: 输入与文件**
- 鼠标/键盘输入注入
- 文件传输 (SCTP)
- 剪贴板同步
