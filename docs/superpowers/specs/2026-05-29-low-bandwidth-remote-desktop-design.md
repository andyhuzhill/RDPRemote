# 低带宽远程桌面控制系统设计文档

**日期**: 2026-05-29  
**项目**: RDPRemote  
**目标**: 跨平台远程控制软件，针对 1Mbps 低带宽场景优化

---

## 1. 协议调研总结

### 1.1 协议对比

| 维度 | RDP | VNC/RFB | TeamViewer | WebRTC |
|------|-----|---------|------------|--------|
| **传输层** | TCP + UDP (10+) | TCP | UDP + TCP | UDP (SRTP+SCTP) |
| **NAT穿透** | 需网关/VPN | 需手动配置 | 自动 (中继+P2P) | 自动 (ICE/STUN/TURN) |
| **编码方式** | H.264/AVC + RemoteFX | RRE/Hextile/ZRLE | 自研视频编码 | VP9(首选) + H.264(降级), 硬件加速 |
| **延迟** | 中 (50-150ms) | 高 (100-300ms) | 低 (30-80ms) | 低 (20-60ms) |
| **带宽效率** | 中 | 低 | 高 | 极高 |
| **浏览器支持** | 需插件 | 需插件 | 否 | 原生支持 |
| **开源可用** | FreeRDP | TigerVNC | 否 | libwebrtc |

### 1.2 1Mbps 带宽可行性分析

可用带宽: 1 Mbps = 128 KB/s
扣除开销后视频流可用: ~105 KB/s ≈ 840 Kbps

分辨率/帧率可行性:
- 1920x1080 @ 30fps -> 不可行 (需 ~8Mbps)
- 1280x720 @ 15fps -> 可行 (需 ~800Kbps)
- 1024x768 @ 15fps -> 舒适 (需 ~500Kbps)
- 800x600 @ 10fps -> 舒适 (需 ~250Kbps)

### 1.3 结论

**推荐方案**: WebRTC + VP9 首选 + H.264 降级

理由:
- ✅ 自适应码率控制 (GCC/BWE)
- ✅ 内置 NAT 穿透 (ICE/STUN/TURN)
- ✅ VP9 低码率画质优于 H.264 (同码率提升 30-50%)
- ✅ 硬件编码 (NVENC/AMF/QSV) 降低延迟至 1-5ms
- ✅ H.264 硬件编码作为降级路径，覆盖存量 GPU
- ✅ 开源可用 (libwebrtc / mediasoup / coturn)

---

## 2. 系统架构设计

### 2.1 整体架构

信令服务器 (WebSocket)
- 用户认证 (JWT)
- 设备注册/发现
- SDP 信令交换
- 会话管理

控制端 (Windows/Linux) <--WebRTC--> 被控端 (Windows)
- 屏幕捕获/渲染
- 输入注入/捕获
- 文件传输

### 2.2 组件说明

| 组件 | 技术选型 | 说明 |
|------|----------|------|
| **信令服务器** | Go / Node.js | 会话管理、SDP 交换 |
| **媒体中继** | coturn (TURN/STUN) | NAT 穿透、中继转发 |
| **被控端 (Windows)** | Rust + Windows API | 屏幕捕获、输入注入 |
| **控制端 (跨平台)** | Rust + Tauri | 屏幕渲染、输入捕获 |
| **WebRTC 引擎** | libwebrtc (C++ FFI) | 支持硬件编码、Simulcast、GCC |

---

## 3. 低带宽优化策略

### 3.1 视频编码优化

编码策略 (自适应):
- **主选编码**: VP9
  - VP9 在低码率下画质优于 H.264 30-50%，SVC 特性天然支持分层自适应
  - 仅在硬件支持时启用（Turing+ NVENC / RDNA 2+ AMF / Xe+ QSV）
- **降级编码**: H.264
  - 当 VP9 硬件编码不可用时自动回退（Pascal NVENC / GCN AMF / 6-10代 QSV）
  - 覆盖存量 GPU 设备，保证最低延迟
- **运行时协商**: 连接建立时通过 WebRTC `RTCRtpSender.getCapabilities` 查询双方编码能力，优先 VP9 硬件 → H.264 硬件 → 软件 VP9 → 软件 H.264
- **硬件编码优先级**: 
  - NVIDIA: NVENC VP9 (Turing+, ≥GTX 1660/RTX 2060) → NVENC H.264 (Pascal, GTX 1050+) → 软件
  - AMD: AMF VP9 (RDNA 2+, RX 6000+) → AMF H.264 (GCN+, RX 400+) → 软件
  - Intel: QSV VP9 (Xe, 11代+) → QSV H.264 (6代+) → 软件
- 静态画面 (文字/桌面): Intra 帧 + 高 QP, 目标码率 200-400 Kbps
- 动态画面 (视频/动画): P/B 帧 + 运动补偿, 目标码率 600-800 Kbps
- 极端低带宽 (<500K): SVC 降级 + 分辨率降级 + 帧率降级, 目标 800x600 @ 8fps

### 3.2 分辨率/帧率自适应

带宽自适应策略:
- High (> 2 Mbps): 1080p @ 30fps
- Medium (1-2 Mbps): 720p @ 15fps
- Low (500K-1Mbps): 720p @ 10fps
- VeryLow (< 500K): 480p @ 8fps

### 3.3 区域裁剪 (ROI)

只传输变化区域，节省带宽 30-70%。

算法方案:
- 变化检测: 2x2 tile hash 比较（类似 NoMachine 方案）
- 变化区域合并/矩形化，减少 Intra 帧碎片
- 变化区域使用 VP9 lossless 模式编码
- 全屏基础帧定时刷新（每 5s 一次），防止累积误差

### 3.4 帧跳过策略

- 连续 N 帧无变化 -> 跳过编码
- 检测到文字界面 -> 降低帧率至 5fps
- 检测到视频播放 -> 保持 15fps
节省带宽: 60-90% (静态场景)

---

## 4. 技术选型

### 4.1 界面框架：Tauri + Rust 原生渲染

选择理由:
- WebView 原生硬件加速，UI 层渲染流畅
- Rust 后端直接调用系统 API (DXGI、SendInput、NVENC)
- 打包体积小 (~5-10MB)
- 跨平台 (Windows/Linux/macOS)

视频渲染策略:
- **方案**: Rust 侧 wgpu 原生渲染，零拷贝
- 避免 WebView IPC 瓶颈（libwebrtc 解码帧 → wgpu 纹理直接渲染）
- WebView 仅负责 UI 面板（连接列表、设置等）
- 视频窗口通过 wgpu 叠加层实现

### 4.2 被控端 (Windows)

| 模块 | 技术 | 说明 |
|------|------|------|
| **GUI** | 纯后台服务 (Windows Service) | 无需 GUI 框架，配置通过本地 Web UI |
| **屏幕捕获** | DXGI Desktop Duplication API | Windows 原生，高性能 |
| **输入注入** | SendInput / UI Automation | 模拟鼠标键盘输入（注：无法注入 UAC 安全桌面） |
| **WebRTC** | libwebrtc (C++ FFI via cxx/bindgen) | 支持硬件编码 |
| **硬件编码** | NVENC > AMF > QSV > 软件 VP9 | 运行时检测 GPU 动态选择 |
| **语言** | Rust + C++ (libwebrtc FFI) | 性能关键路径用 Rust |

### 4.3 控制端 (Windows/Linux)

| 模块 | 技术 | 说明 |
|------|------|------|
| **GUI** | Tauri + React/Vue | 跨平台桌面应用 |
| **WebRTC** | libwebrtc (C++ FFI) | 视频解码 |
| **视频渲染** | Rust 侧 wgpu 原生渲染（零拷贝） | 避免 WebView IPC 瓶颈 |
| **输入捕获** | 平台原生 API | Windows: Raw Input, Linux: libevdev (X11) / libei (Wayland) |
| **语言** | Rust + TypeScript + C++ FFI | 统一技术栈 |

### 4.4 服务器端

| 模块 | 技术 | 说明 |
|------|------|------|
| **信令服务器** | Go / Node.js | WebSocket 信令 |
| **TURN/STUN** | coturn | 开源，稳定 |
| **数据库** | SQLite / PostgreSQL | 用户/设备管理 |
| **部署** | Docker | 容器化部署 |

---

## 5. 核心流程

**备注**: 本系统不包含音频传输，全部带宽用于视频流和控制信令。

### 5.1 连接建立流程

1. 被控端启动 -> 注册设备到信令服务器
2. 控制端发起连接 -> 通过信令服务器查找被控端
3. WebRTC 信令交换 (offer/answer + ICE)
4. P2P 直连或 TURN 中继

### 5.2 屏幕传输流程

被控端: DXGI 屏幕捕获 -> 变化检测 -> 编码 -> WebRTC 发送
控制端: WebRTC 接收 -> 解码 -> 渲染 -> 输入捕获 -> WebRTC 发送

---

## 6. 安全设计

| 安全层 | 措施 |
|--------|------|
| **传输加密** | DTLS-SRTP (WebRTC 原生) |
| **信令加密** | WSS (WebSocket over TLS) |
| **身份认证** | JWT + 设备指纹 |
| **访问控制** | 一次性连接码 / 白名单 |
| **审计日志** | 连接记录、操作日志 |

---

## 7. 项目结构

RDPRemote/
  agent/           # 被控端 (Windows)
  client/          # 控制端 (跨平台)
  server/          # 信令服务器
  turn/            # TURN 服务器配置
  docs/
    specs/
  Cargo.toml       # Workspace

---

## 8. 开发计划

### Phase 1a: 局域网核心验证 (4 周)
- 本地 P2P WebRTC 连接建立 (libwebrtc)
- Windows 屏幕捕获 (DXGI)
- VP9 硬件编码 (NVENC) + CPU 编码兜底
- Rust 侧 wgpu 渲染 (控制端)

### Phase 1b: 网络穿透 + 自适应 (4 周)
- STUN/TURN (coturn) 集成
- WebRTC GCC 自适应码率
- 带宽自适应切换 (720p@15fps → 480p@8fps)

### Phase 2: 低带宽优化 (3 周)
- 区域裁剪 (ROI + tile hash)
- 帧跳过策略
- 分辨率动态调整
- VP9 SVC 分层编码

### Phase 3: 输入与文件 (2 周)
- 鼠标/键盘输入注入
- 文件传输 (SCTP 数据通道)

### Phase 4: 生产准备 (2-3 周)
- 信令服务器
- TURN 服务器部署
- 安全认证
- 打包发布

---

## 9. 待确认事项

1. **WebRTC 引擎选择**: 已确定使用 libwebrtc (C++ FFI)
   - Rust 侧通过 `cxx` 或 `bindgen` 封装
   - 需评估 webrtc-rs 后续成熟度，未来可替换

2. **是否需要 libwebrtc Simulcast**: 
   - VP9 SVC (可分级视频编码) 可替代 Simulcast 功能
   - 1Mbps 场景下仅需 1 层编码 + 自适应降级

3. **GPU 编码兼容性**:
   - **VP9 硬件编码**: NVIDIA Turing+ (GTX 1660/RTX 2060+), AMD RDNA 2+ (RX 6000+), Intel Xe+ (11代+)
   - **H.264 硬件编码 (降级)**: NVIDIA Pascal+ (GTX 1050+), AMD GCN+ (RX 400+), Intel 6代+
   - 运行时通过 WebRTC 能力协商自动选择

4. **目标平台**:
   - 被控端: Windows 10/11 (最低 Win8+，因 DXGI Desktop Duplication 依赖)
   - 控制端: Windows 10/11 + Linux

5. **剪贴板同步**:
   - 使用 WebRTC DataChannel (SCTP) 传输
   - 格式支持: 文本 → HTML → 图片
   - Phase 3 实现

---

## 10. 参考资源

- [WebRTC 官方文档](https://webrtc.org/)
- [libwebrtc (C++ 实现)](https://github.com/webrtc-sdk/libwebrtc)
- [webrtc-rs (Rust 实现, 评估中)](https://github.com/webrtc-rs/webrtc)
- [coturn TURN 服务器](https://github.com/coturn/coturn)
- [NVENC 硬件编码 SDK](https://developer.nvidia.com/video-codec-sdk)
- [FFmpeg VP9 编码指南](https://trac.ffmpeg.org/wiki/Encode/VP9)
- [DXGI Desktop Duplication](https://learn.microsoft.com/en-us/windows/win32/direct3d11/dxgi-desktop-duplication)
- [Tauri 文档](https://tauri.app/)
- [wgpu Rust GPU 渲染](https://wgpu.rs/)
