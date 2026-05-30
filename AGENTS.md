# AGENTS.md

## Project

RDPRemote - Cross-platform remote desktop control system using WebRTC + VP9 encoding.

## Build

```bash
cargo build          # Build all crates
cargo check          # Type-check only (faster)
cargo test           # Run all tests
cargo run -p rdp-server   # Start signaling server
cargo run -p rdp-client -- --target-agent agent1  # Start client
```

Agent (`rdp-agent`) requires Windows for screen capture. On Linux, only `cargo check` works for agent.

## Architecture

```
common/   → Shared types: SignalingMessage, VideoFrameHeader
server/   → WebSocket signaling server (port 8765)
agent/    → Windows-only: DXGI capture + VP9 encode + WebRTC send
client/   → Cross-platform: WebRTC receive + wgpu render
```

## Key Crates

- `webrtc` (agent uses 0.10, client uses 0.12) - version mismatch is intentional
- `libvpx-sys` - VP9 software encoding
- `windows` - DXGI Desktop Duplication (Windows-only, behind `cfg(target_os = "windows")`)
- `wgpu`/`winit` - Video rendering on client

## Code Conventions

- Workspace dependencies in root `Cargo.toml`
- Agent uses `#[cfg(target_os = "windows")]` for Windows-only code
- Error handling: `anyhow::Result` throughout
- Async runtime: tokio with specific features per crate
- Logging: `tracing` + `tracing-subscriber`

## Common Issues

- Agent won't compile on Linux without `--target x86_64-pc-windows-msvc`
- `webrtc` crate versions differ between agent (0.10) and client (0.12) - don't unify
- SplitSink cannot be cloned - use mpsc channels to forward messages
- ICE candidates collected via `on_ice_candidate` callback, sent through signaling server

## File Structure

- `agent/src/screen/dxgi.rs` - DXGI screen capture (Windows-only)
- `agent/src/encoder/vp9.rs` - VP9 encoder via libvpx
- `agent/src/adaptive.rs` - Bandwidth adaptive controller
- `agent/src/roi.rs` - Region of Interest (tile hash)
- `agent/src/frame_skipper.rs` - Frame skipping strategy
- `agent/src/input.rs` - Input injection (SendInput)
- `agent/src/webrtc/peer.rs` - WebRTC sender peer
- `client/src/webrtc/peer.rs` - WebRTC receiver peer
- `client/src/render/wgpu_renderer.rs` - wgpu video renderer
- `common/src/signaling.rs` - Signaling message types
- `common/src/file_transfer.rs` - File transfer state machine
- `common/src/clipboard.rs` - Clipboard manager
- `server/src/auth.rs` - JWT authentication

## Deployment

```bash
# Docker
docker-compose up -d

# Manual
cargo run -p rdp-server
cargo run -p rdp-agent -- --device-id my-pc
cargo run -p rdp-client -- --target-agent my-pc
```

JWT secret via `JWT_SECRET` env var.

## Phase Status

- Phase 1a: Core pipeline (capture → encode → WebRTC → render) ✅
- Phase 1b: Network traversal + adaptive ✅
- Phase 2: Low-bandwidth optimization (ROI, frame skipping) ✅
- Phase 3: Input injection + file transfer ✅
- Phase 4: Production ready (JWT auth, Docker, docs) ✅
