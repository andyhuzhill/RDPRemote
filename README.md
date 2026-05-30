# RDPRemote

> 跨平台远程桌面控制系统 - 基于 WebRTC + VP9 编码

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)

## 概述

RDPRemote 是一个高性能的跨平台远程桌面控制系统，采用 WebRTC 实时通信协议和 VP9 视频编码技术，实现低延迟、高画质的远程桌面体验。

### 核心特性

- 🚀 **低延迟传输** - 基于 WebRTC 的 P2P 通信，端到端延迟 < 100ms
- 🎬 **VP9 编码** - 高效的视频压缩，节省带宽 30-50%
- 🔐 **端到端加密** - WebRTC 内置 DTLS-SRTP 加密
- 🖥️ **跨平台支持** - Agent 端 Windows，Client 端全平台
- 📋 **剪贴板同步** - 双向剪贴板共享
- 📁 **文件传输** - 支持文件拖拽传输
- 🎯 **ROI 智能编码** - 聚焦区域优先编码，优化低带宽场景

## 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                        RDPRemote Architecture                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐         ┌──────────────┐                     │
│  │   Agent      │         │    Client    │                     │
│  │  (Windows)   │         │ (Cross-PLAT) │                     │
│  ├──────────────┤         ├──────────────┤                     │
│  │ DXGI Capture │◄───────►│  wgpu Render │                     │
│  │    VP9 Enc   │         │  WebRTC Rec  │                     │
│  │  WebRTC Send │         │  Input Inject│                     │
│  └──────┬───────┘         └──────┬───────┘                     │
│         │                        │                              │
│         │    WebSocket (WS)      │                              │
│         ▼                        ▼                              │
│  ┌──────────────────────────────────────────┐                   │
│  │           Signaling Server                │                   │
│  │           (Port 8765)                     │                   │
│  │  • Peer negotiation                       │                   │
│  │  • ICE candidate exchange                 │                   │
│  │  • Session management                     │                   │
│  └──────────────────────────────────────────┘                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## 快速开始

### 环境要求

- Rust 1.75+
- Cargo
- Windows 10/11 (Agent 端)
- Linux / macOS / Windows (Client 端)

### 构建

```bash
# 构建所有 crate
cargo build

# 仅类型检查（更快）
cargo check

# 运行所有测试
cargo test
```

### 启动服务

```bash
# 启动信令服务器
cargo run -p rdp-server

# 启动客户端
cargo run -p rdp-client -- --target-agent agent1
```

## 项目结构

```
RDPRemote/
├── common/           # 共享类型和协议定义
│   ├── src/
│   │   ├── signaling.rs    # 信令消息类型
│   │   ├── protocol.rs     # 通信协议
│   │   └── clipboard.rs    # 剪贴板同步
│   └── tests/
│
├── server/           # WebSocket 信令服务器
│   ├── src/
│   │   ├── main.rs         # 服务器入口
│   │   └── auth.rs         # 认证模块
│   └── tests/
│
├── agent/            # 桌面采集端 (Windows)
│   ├── src/
│   │   ├── screen/
│   │   │   └── dxgi.rs     # DXGI 桌面捕获
│   │   ├── encoder/
│   │   │   └── vp9.rs      # VP9 编码器
│   │   ├── webrtc/
│   │   │   └── peer.rs     # WebRTC 发送端
│   │   ├── adaptive.rs     # 带宽自适应
│   │   └── roi.rs          # ROI 区域优化
│   └── tests/
│
├── client/           # 客户端 (跨平台)
│   ├── src/
│   │   ├── webrtc/
│   │   │   └── peer.rs     # WebRTC 接收端
│   │   └── render/
│   │       └── wgpu_renderer.rs  # wgpu 渲染器
│   └── tests/
│
├── docs/             # 文档
│   ├── ARCHITECTURE.md
│   └── DEPLOYMENT.md
│
├── scripts/          # 脚本工具
├── turn/             # TURN 服务器配置
├── Dockerfile        # Docker 镜像定义
├── docker-compose.yml
└── Cargo.toml        # Workspace 配置
```

## 开发指南

### 核心模块

| 模块 | 描述 | 平台 |
|------|------|------|
| `rdp-agent` | 桌面捕获、编码、发送 | Windows |
| `rdp-client` | 接收、解码、渲染、输入注入 | 全平台 |
| `rdp-server` | WebSocket 信令服务器 | 全平台 |
| `rdp-common` | 共享类型和协议 | 全平台 |

### 关键依赖

- `webrtc` - WebRTC 实现（agent 0.10, client 0.12）
- `libvpx-sys` - VP9 软件编码
- `windows` - DXGI Desktop Duplication (Windows)
- `wgpu` / `winit` - 跨平台图形渲染

### 代码规范

- 使用 `anyhow::Result` 统一错误处理
- 异步运行时使用 `tokio`
- 日志使用 `tracing` + `tracing-subscriber`
- Workspace 依赖统一管理

## 部署

详见 [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)

## 架构设计

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## 开发路线图

| 阶段 | 功能 | 状态 |
|------|------|------|
| Phase 1a | 核心流水线 (捕获→编码→WebRTC→渲染) | ✅ 完成 |
| Phase 1b | 网络穿越 + 自适应控制 | ✅ 完成 |
| Phase 2 | 低带宽优化 (ROI, 帧跳过) | 🔄 进行中 |
| Phase 3 | 输入注入 + 文件传输 | ⏳ 待开发 |

## 常见问题

### Agent 在 Linux 上无法编译

Agent 端使用 DXGI 桌面捕获，仅支持 Windows。在 Linux 上只能进行类型检查：

```bash
cargo check -p rdp-agent
```

### webrtc 版本不一致

Agent 使用 `webrtc 0.10`，Client 使用 `webrtc 0.12`。这是故意的设计，不要统一版本。

### SplitSink 无法克隆

使用 `mpsc` 通道转发消息：

```rust
let (tx, mut rx) = mpsc::channel(100);
// 在多个任务间共享 tx
```

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

---

**RDPRemote Team** - 让远程办公更高效