# Phase 1b: 网络穿透 + 自适应 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 STUN/TURN 网络穿透、WebRTC GCC 自适应码率、自动 SDP 信令交换

**Architecture:** 
- 信令服务器已实现 (server/)，需要扩展支持自动 SDP 转发
- Agent/Client 已实现手动 SDP 流程，需要改为自动 WebSocket 信令
- 添加 coturn 配置用于 NAT 穿透
- 实现带宽自适应策略

**Tech Stack:** Rust, webrtc-rs, tokio-tungstenite, coturn

---

## Task 1: 自动信令交换

**Files:**
- Modify: `agent/src/main.rs`
- Modify: `client/src/main.rs`

将手动 SDP 流程改为自动 WebSocket 信令：

1. Agent 启动后自动注册并等待 Connect
2. Client 启动后自动注册、发送 Connect、等待 Offer
3. 自动完成 Offer/Answer 交换
4. 自动处理 ICE Candidate 转发

- [ ] **Step 1: 修改 agent main.rs 信令逻辑**

Agent 信令流程：
- 连接信令服务器
- 注册设备
- 等待 Connect 消息
- 收到 Connect 后创建 Offer 并发送
- 等待 Answer
- 收到 Answer 后设置远程描述
- 循环处理 ICE Candidate

- [ ] **Step 2: 修改 client main.rs 信令逻辑**

Client 信令流程：
- 连接信令服务器
- 注册设备
- 发送 Connect 到目标 Agent
- 等待 Offer
- 收到 Offer 后创建 Answer 并发送
- 循环处理 ICE Candidate

- [ ] **Step 3: 验证编译**

Run: `cargo build`
Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "feat: automatic signaling exchange"
```

---

## Task 2: ICE Candidate 自动收集和转发

**Files:**
- Modify: `agent/src/webrtc/peer.rs`
- Modify: `client/src/webrtc/peer.rs`
- Modify: `agent/src/main.rs`
- Modify: `client/src/main.rs`

实现 ICE Candidate 自动收集和通过信令服务器转发：

1. 在 PeerConnection 上注册 on_ice_candidate 回调
2. 收集到的 ICE Candidate 通过 WebSocket 发送到信令服务器
3. 信令服务器转发到对端
4. 对端收到后添加到 PeerConnection

- [ ] **Step 1: 修改 AgentPeer 添加 ICE 回调**

```rust
pub fn on_ice_candidate<F>(&self, callback: F)
where
    F: Fn(String, String, u16) + Send + Sync + 'static,
{
    let pc = self.peer_connection.clone();
    // 注册 on_ice_candidate 回调
}
```

- [ ] **Step 2: 修改 ClientPeer 添加 ICE 回调**

同上

- [ ] **Step 3: 在 main.rs 中集成 ICE 转发**

Agent/Client 收集到 ICE Candidate 后通过 WebSocket 发送

- [ ] **Step 4: 验证编译并提交**

---

## Task 3: STUN/TURN 配置

**Files:**
- Create: `turn/turnserver.conf`
- Modify: `agent/src/webrtc/peer.rs` (添加 ICE 服务器配置)
- Modify: `client/src/webrtc/peer.rs` (添加 ICE 服务器配置)

配置 TURN 服务器用于 NAT 穿透：

- [ ] **Step 1: 创建 turnserver.conf**

```conf
listening-port=3478
tls-listening-port=5349
fingerprint
lt-cred-mech
user=rdpremote:rdpremote123
realm=rdpremote.local
total-quota=100
bps-capacity=0
stale-nonce=600
```

- [ ] **Step 2: 修改 PeerConnection 配置**

```rust
let config = RTCConfiguration {
    ice_servers: vec![
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["turn:your-server:3478".to_string()],
            username: "rdpremote".to_string(),
            credential: "rdpremote123".to_string(),
            ..Default::default()
        },
    ],
    ..Default::default()
};
```

- [ ] **Step 3: 验证编译并提交**

---

## Task 4: 带宽自适应策略

**Files:**
- Modify: `agent/src/main.rs`
- Create: `agent/src/adaptive.rs` (可选)

实现带宽自适应：

1. 监控 WebRTC 统计信息 (RTCP Receiver Report)
2. 根据丢包率和 RTT 调整编码参数
3. 动态调整分辨率和帧率

- [ ] **Step 1: 定义自适应策略**

```rust
enum BandwidthTier {
    High,      // > 2 Mbps: 1080p @ 30fps
    Medium,    // 1-2 Mbps: 720p @ 15fps
    Low,       // 500K-1Mbps: 720p @ 10fps
    VeryLow,   // < 500K: 480p @ 8fps
}
```

- [ ] **Step 2: 实现带宽检测**

通过 WebRTC stats API 获取当前码率

- [ ] **Step 3: 实现编码参数动态调整**

根据带宽层级调整 VP9 编码器参数

- [ ] **Step 4: 验证编译并提交**

---

## 验证清单

- [ ] `cargo build` 编译成功
- [ ] Agent 和 Client 可以自动完成信令交换
- [ ] ICE Candidate 自动收集和转发
- [ ] STUN/TURN 配置正确
- [ ] 带宽自适应策略实现
