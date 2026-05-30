# RDPRemote 架构文档

> 系统架构设计与技术细节

## 目录

- [系统概述](#系统概述)
- [架构设计](#架构设计)
- [数据流](#数据流)
- [核心模块](#核心模块)
- [协议设计](#协议设计)
- [性能优化](#性能优化)
- [安全设计](#安全设计)

---

## 系统概述

RDPRemote 是一个基于 WebRTC 的跨平台远程桌面控制系统，采用微服务架构设计，支持低延迟、高画质的远程桌面体验。

### 设计目标

| 目标 | 指标 |
|------|------|
| 端到端延迟 | < 100ms |
| 视频画质 | 1080p @ 30fps |
| 带宽占用 | 1-5 Mbps (自适应) |
| CPU 占用 | Agent < 15%, Client < 10% |
| 并发连接 | 单服务器 1000+ |

---

## 架构设计

### 系统架构图

```mermaid
graph TB
    subgraph Agent["Agent (Windows)"]
        A1[DXGI Screen Capture]
        A2[VP9 Encoder]
        A3[WebRTC Sender]
        A4[Input Handler]
        A5[Clipboard Sync]
        A1 --> A2
        A2 --> A3
        A4 --> A1
        A5 -.-> A3
    end

    subgraph Network["Network"]
        N1[WebSocket Signaling]
        N2[WebRTC P2P]
        N3[TURN Server]
    end

    subgraph Server["Signaling Server"]
        S1[Connection Manager]
        S2[Session Manager]
        S3[ICE Exchange]
    end

    subgraph Client["Client (Cross-Platform)"]
        C1[WebRTC Receiver]
        C2[wgpu Renderer]
        C3[Input Injector]
        C4[Clipboard Sync]
        C1 --> C2
        C3 -.-> C1
        C4 -.-> C1
    end

    Agent -->|WS: Offer/Answer| Server
    Agent -->|ICE Candidates| Server
    Server -->|WS: Offer/Answer| Client
    Server -->|ICE Candidates| Client
    Agent -->|WebRTC Stream| N2
    N2 -->|P2P / Relay| Client
    N3 -.->|Fallback| N2

    style Agent fill:#e1f5fe
    style Client fill:#f3e5f5
    style Server fill:#e8f5e9
    style Network fill:#fff3e0
```

### 组件交互图

```mermaid
sequenceDiagram
    participant A as Agent
    participant S as Signaling Server
    participant C as Client
    participant T as TURN Server

    A->>S: WebSocket Connect
    C->>S: WebSocket Connect
    S->>A: Session ID
    S->>C: Session ID

    A->>A: Create WebRTC PeerConnection
    A->>A: Create Offer
    A->>S: Send Offer
    S->>C: Forward Offer
    C->>C: Create Answer
    C->>S: Send Answer
    S->>A: Forward Answer

    A->>S: ICE Candidate
    C->>S: ICE Candidate
    S->>C: Forward ICE
    S->>A: Forward ICE

    A->>C: WebRTC Connection Established
    C->>C: Start Rendering

    loop Video Stream
        A->>A: Capture Screen (DXGI)
        A->>A: Encode (VP9)
        A->>C: Send Frame (WebRTC)
        C->>C: Decode & Render (wgpu)
    end

    C->>A: Input Events (Keyboard/Mouse)
    A->>A: Inject Input
```

---

## 数据流

### 视频流管道

```mermaid
flowchart LR
    subgraph Capture["屏幕捕获"]
        D[Desktop] -->|DXGI| DC[DXGI Duplication]
        DC -->|Frame| BF[Buffer]
    end

    subgraph Process["处理"]
        BF -->|Raw| RP[Resize/Padding]
        RP -->|I420| EN[VP9 Encoder]
    end

    subgraph Encode["编码"]
        EN -->|Config| CF[Config Frame]
        EN -->|Data| DF[Data Frame]
        CF -->|Header| HF[Header]
        DF -->|Payload| PF[Payload]
    end

    subgraph Transport["传输"]
        HF -->|Signaling| WS[WebSocket]
        PF -->|RTP| RT[WebRTC RTP]
    end

    subgraph Receive["接收"]
        RT -->|Packet| DP[Decoder]
        DP -->|I420| WR[wgpu Renderer]
        WR -->|Texture| SC[Screen]
    end

    style Capture fill:#bbdefb
    style Process fill:#c8e6c9
    style Encode fill:#fff9c4
    style Transport fill:#ffe0b2
    style Receive fill:#e1bee7
```

### 控制流管道

```mermaid
flowchart TB
    subgraph Input["输入"]
        K[Keyboard] --> IE[Input Event]
        M[Mouse] --> IE
        IE --> QE[Queue]
    end

    subgraph Serialize["序列化"]
        QE --> SB[Serialize]
        SB --> PB[Packet Build]
    end

    subgraph Send["发送"]
        PB -->|WebSocket| WS[Signaling]
        PB -->|WebRTC Data| DT[Data Channel]
    end

    subgraph Handle["处理"]
        WS -->|Agent| AH[Agent Handler]
        DT -->|Direct| DH[Direct Handler]
        AH --> IN[Inject]
        DH --> IN
    end

    IN -->|Windows| WIN[SendInput]
    IN -->|Linux| LX[xdotool/X11]
    IN -->|macOS| MAC[CGEvent]

    style Input fill:#bbdefb
    style Serialize fill:#c8e6c9
    style Send fill:#fff9c4
    style Handle fill:#ffe0b2
```

---

## 核心模块

### Agent 模块

```mermaid
classDiagram
    class ScreenCapture {
        +capture_frame() Frame
        +get_resolution() Resolution
        +start() void
        +stop() void
    }

    class DXDICapture {
        -device: IDXGIDevice
        -output: IDXGIOutput
        -acquire_next_frame() Result
        +capture() Frame
    }

    class VP9Encoder {
        -ctx: vpx_codec_ctx_t
        +encode(frame) CompressedFrame
        +set_bitrate(bps) void
        +set_quality(quality) void
    }

    class WebRTCPeer {
        -peer_connection: RTCPeerConnection
        +create_offer() SessionDescription
        +set_remote_description(desc) void
        +add_ice_candidate(candidate) void
        +on_track(handler) void
    }

    class AdaptiveController {
        -bandwidth_estimator: BandwidthEstimator
        +update_rate(frame) void
        +get_target_bitrate() u32
        +should_skip_frame() bool
    }

    ScreenCapture <|-- DXDICapture
    ScreenCapture --> VP9Encoder
    VP9Encoder --> WebRTCPeer
    VP9Encoder --> AdaptiveController

    note for DXDICapture "Windows only"
```

### Client 模块

```mermaid
classDiagram
    class WebRTCReceiver {
        -peer_connection: RTCPeerConnection
        +create_answer() SessionDescription
        +on_track(track) void
        +receive_frame() Frame
    }

    class WgpuRenderer {
        -device: wgpu::Device
        -queue: wgpu::Queue
        -pipeline: RenderPipeline
        +render(frame) void
        +resize(width, height) void
    }

    class InputInjector {
        +inject_mouse(x, y, buttons) void
        +inject_keyboard(key, modifiers) void
        +inject_scroll(delta) void
    }

    class ClipboardManager {
        -sync_channel: Channel
        +sync_clipboard() void
        +on_clipboard_update(data) void
    }

    WebRTCReceiver --> WgpuRenderer
    WebRTCReceiver --> InputInjector
    WebRTCReceiver --> ClipboardManager

    class VideoDecoder {
        <<interface>>
        +decode(packet) Frame
    }

    class VP9SoftwareDecoder {
        -- implements VideoDecoder
        +decode(packet) Frame
    }

    VideoDecoder <|.. VP9SoftwareDecoder
    WgpuRenderer --> VideoDecoder
```

---

## 协议设计

### 信令消息协议

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Connecting: connect()
    Connecting --> Connected: on_open()
    Connecting --> Failed: on_error()

    Connected --> SendingOffer: create_offer()
    SendingOffer --> WaitingAnswer: send_offer()
    WaitingAnswer --> Connected: receive_answer()

    Connected --> SendingICE: collect_ice()
    SendingICE --> Connected: exchange_ice()

    Connected --> Disconnected: close()
    Disconnected --> [*]

    Failed --> [*]
```

### SignalingMessage 定义

```rust
// common/src/signaling.rs

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    // 连接管理
    Connect {
        agent_id: String,
    },

    // WebRTC 信令
    Offer {
        sdp: String,
    },

    Answer {
        sdp: String,
    },

    IceCandidate {
        candidate: String,
        sdp_mline_index: u32,
        sdp_mid: String,
    },

    // 会话控制
    Ready,
    Start,
    Stop,

    // 错误处理
    Error {
        code: ErrorCode,
        message: String,
    },
}
```

### 视频帧协议

```mermaid
flowchart LR
    subgraph Header["Frame Header (16 bytes)"]
        H1[Magic: 0x52445046]
        H2[Version: u8]
        H3[Flags: u8]
        H4[Width: u16]
        H5[Height: u16]
        H6[Timestamp: u64]
        H7[FrameType: u8]
        H8[PayloadLen: u32]
    end

    subgraph Payload["VP9 Payload"]
        P1[VP9 Frame Data]
    end

    Header --> Payload

    note right of Header
        - I-Frame: Key frame
        - P-Frame: Predictive frame
        - ROI: Region of interest
    end note
```

---

## 性能优化

### 带宽自适应

```mermaid
graph LR
    subgraph Monitor["带宽监控"]
        M1[RTT Monitor]
        M2[Packet Loss]
        M3[Bitrate Estimator]
    end

    subgraph Controller["自适应控制器"]
        C1[Target Bitrate]
        C2[Frame Skip]
        C3[Resolution Scale]
    end

    subgraph Encoder["编码器调整"]
        E1[VP9 Quality]
        E2[Keyframe Interval]
    end

    M1 --> C1
    M2 --> C1
    M3 --> C1
    C1 --> C2
    C1 --> C3
    C1 --> E1
    C2 --> E2

    style Monitor fill:#bbdefb
    style Controller fill:#c8e6c9
    style Encoder fill:#fff9c4
```

### ROI 区域优化

```mermaid
flowchart TB
    subgraph Detect["运动检测"]
        D1[Previous Frame]
        D2[Current Frame]
        D3[Difference]
        D4[Motion Regions]
    end

    subgraph Prioritize["优先级分配"]
        P1[High Priority: ROI]
        P2[Medium Priority: Surround]
        P3[Low Priority: Background]
    end

    subgraph Encode["差异化编码"]
        E1[ROI: High Quality]
        E2[Surround: Medium]
        E3[Background: Low/Skip]
    end

    D1 --> D3
    D2 --> D3
    D3 --> D4
    D4 --> P1
    D4 --> P2
    D4 --> P3
    P1 --> E1
    P2 --> E2
    P3 --> E3

    style Detect fill:#bbdefb
    style Prioritize fill:#c8e6c9
    style Encode fill:#fff9c4
```

---

## 安全设计

### 安全架构

```mermaid
graph TB
    subgraph Transport["传输安全"]
        T1[DTLS 1.3]
        T2[SRTP]
        T3[Key Exchange]
    end

    subgraph Signaling["信令安全"]
        S1[TLS 1.3]
        S2[Authentication]
        S3[Session Token]
    end

    subgraph Application["应用安全"]
        A1[Access Control]
        A2[Rate Limiting]
        A3[Audit Log]
    end

    T1 --> T2
    T3 --> T1
    S1 --> S2
    S2 --> S3
    A1 --> A2
    A2 --> A3

    style Transport fill:#ffcdd2
    style Signaling fill:#fff9c4
    style Application fill:#c8e6c9
```

### 认证流程

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server
    participant A as Auth Service

    C->>S: Connect Request
    S->>A: Validate Token
    A-->>S: Token Valid
    S->>C: Session Token
    C->>S: Authenticated Connect
    S->>C: Connection Accepted

    Note over C,S: All signaling encrypted with TLS
    Note over C,S: WebRTC uses DTLS-SRTP
```

---

## 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| 运行时 | Rust 1.75+ | 内存安全、高性能 |
| 异步框架 | tokio | 异步 I/O |
| WebRTC | webrtc-rs | P2P 通信 |
| 视频编码 | libvpx | VP9 编码 |
| 屏幕捕获 | DXGI | Windows 桌面复制 |
| 图形渲染 | wgpu | 跨平台 GPU 渲染 |
| 序列化 | serde | 高效序列化 |
| 日志 | tracing | 结构化日志 |

---

## 扩展性设计

### 水平扩展

```mermaid
graph TB
    LB[Load Balancer] --> S1[Signaling Server 1]
    LB --> S2[Signaling Server 2]
    LB --> S3[Signaling Server 3]

    S1 --> DB[(Redis Cluster)]
    S2 --> DB
    S3 --> DB

    S1 --> Q1[(Message Queue)]
    S2 --> Q1
    S3 --> Q1

    style LB fill:#bbdefb
    style DB fill:#c8e6c9
    style Q1 fill:#fff9c4
```

### 插件架构

```mermaid
classDiagram
    class Plugin {
        <<interface>>
        +on_connect(session) void
        +on_disconnect(session) void
        +on_message(msg) Message
    }

    class AuthPlugin {
        +validate_token(token) bool
    }

    class MetricsPlugin {
        +record_metric(name, value) void
    }

    class LoggingPlugin {
        +log_event(event) void
    }

    Plugin <|.. AuthPlugin
    Plugin <|.. MetricsPlugin
    Plugin <|.. LoggingPlugin

    class PluginManager {
        -plugins: Vec~Plugin~
        +register(plugin) void
        +execute_hooks(event) void
    }

    PluginManager --> Plugin
```

---

*最后更新: 2026-05-30*