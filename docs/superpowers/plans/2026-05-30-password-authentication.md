# 密码验证机制 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现密码验证机制，控制端必须输入正确密码才能连接被控端

**Architecture:** 
- 在信令协议中添加 AuthRequest/AuthResponse 消息类型
- agent 端维护设备密码映射表，验证客户端密码
- 客户端在连接前发送密码认证请求
- 密码验证成功后才建立 WebRTC 连接

**Tech Stack:** Rust, tokio, serde, rand (密码生成)

---

## 文件变更清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `common/src/signaling.rs` | 修改 | 添加 AuthRequest 和 AuthResponse 消息类型 |
| `agent/src/auth.rs` | 创建 | 设备认证管理器 DeviceAuthManager |
| `agent/src/main.rs` | 修改 | 集成密码验证逻辑 |
| `client/src/main.rs` | 修改 | 添加密码参数，发送认证请求 |
| `agent/Cargo.toml` | 修改 | 添加 rand 依赖 |

---

## Tasks

### Task 1: 修改信令协议 - 添加 AuthRequest 消息

**Files:**
- Modify: `common/src/signaling.rs:20-24`

- [ ] **Step 1: 修改 SignalingMessage 枚举**

将现有的 `Auth` 变体替换为 `AuthRequest` 和 `AuthResponse`：

```rust
// Authentication messages
#[serde(rename = "auth-request")]
AuthRequest {
    device_id: String,
    password: String,
},
#[serde(rename = "auth-response")]
AuthResponse {
    success: bool,
    message: Option<String>,
},
```

- [ ] **Step 2: 运行测试验证编译**

```bash
cargo check -p rdp-common
```

Expected: PASS (编译通过)

---

### Task 2: 创建设备认证管理器

**Files:**
- Create: `agent/src/auth.rs`

- [ ] **Step 1: 编写测试代码**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_verify() {
        let manager = DeviceAuthManager::new();
        manager.register_device("device-1".to_string(), "123456".to_string()).await;
        
        assert!(manager.verify_password("device-1", "123456").await);
        assert!(!manager.verify_password("device-1", "wrong").await);
        assert!(!manager.verify_password("unknown", "123456").await);
    }

    #[test]
    fn test_generate_password() {
        let password = DeviceAuthManager::generate_password();
        assert_eq!(password.len(), 6);
        assert!(password.chars().all(|c| c.is_ascii_digit()));
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p rdp-agent auth -- --nocapture
```

Expected: FAIL (编译错误，因为 auth.rs 不存在)

- [ ] **Step 3: 实现 DeviceAuthManager**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 设备认证管理器
pub struct DeviceAuthManager {
    /// 设备代码 -> 密码
    devices: Arc<RwLock<HashMap<String, String>>>,
}

impl DeviceAuthManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册设备
    pub async fn register_device(&self, device_id: String, password: String) {
        let mut devices = self.devices.write().await;
        devices.insert(device_id, password);
    }
    
    /// 验证密码
    pub async fn verify_password(&self, device_id: &str, password: &str) -> bool {
        let devices = self.devices.read().await;
        match devices.get(device_id) {
            Some(stored_password) => stored_password == password,
            None => false,
        }
    }
    
    /// 生成随机密码
    pub fn generate_password() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let password: String = (0..6)
            .map(|_| rng.gen_range(0..10).to_string())
            .collect();
        password
    }
}
```

- [ ] **Step 4: 运行测试验证通过**

```bash
cargo test -p rdp-agent auth -- --nocapture
```

Expected: PASS (2 tests pass)

---

### Task 3: 修改 agent/main.rs 集成密码验证

**Files:**
- Modify: `agent/src/main.rs`

- [ ] **Step 1: 添加 auth 模块声明和依赖**

在文件顶部添加：

```rust
#[cfg(target_os = "windows")]
mod auth;
#[cfg(target_os = "windows")]
use auth::DeviceAuthManager;
```

- [ ] **Step 2: 在 run_agent 函数中初始化认证管理器**

在 `run_agent` 函数开始处添加：

```rust
let auth_manager = Arc::new(DeviceAuthManager::new());
let password = DeviceAuthManager::generate_password();
auth_manager.register_device(args.device_id.clone(), password.clone()).await;

// 显示密码
tracing::info!("设备代码: {}", args.device_id);
tracing::info!("连接密码: {}", password);
```

- [ ] **Step 3: 在信令循环中添加密码验证**

在 `while !connected` 循环中，处理 `AuthRequest` 消息：

```rust
Ok(SignalingMessage::AuthRequest { device_id, password }) => {
    tracing::info!("Auth request from: {}", device_id);
    let auth_manager = auth_manager.clone();
    if auth_manager.verify_password(&device_id, &password).await {
        tracing::info!("Authentication successful for: {}", device_id);
        tx.send(SignalingMessage::AuthResponse { success: true, message: None }).await?;
    } else {
        tracing::warn!("Authentication failed for: {}", device_id);
        tx.send(SignalingMessage::AuthResponse { 
            success: false, 
            message: Some("密码错误".to_string()) 
        }).await?;
        continue; // 跳过后续连接流程
    }
}
```

- [ ] **Step 4: 编译验证**

```bash
cargo check -p rdp-agent
```

Expected: PASS

---

### Task 4: 修改 client/main.rs 添加密码认证

**Files:**
- Modify: `client/src/main.rs`

- [ ] **Step 1: 添加密码命令行参数**

修改 Args 结构体：

```rust
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "ws://localhost:8765")]
    server: String,

    #[arg(short, long, default_value = "client-1")]
    device_id: String,

    #[arg(short, long, required = true)]
    target_agent: String,

    #[arg(short, long, required = true)]
    password: String,  // 新增密码参数
}
```

- [ ] **Step 2: 在连接前发送认证请求**

在发送 `Connect` 消息之前添加认证逻辑：

```rust
// 发送认证请求
let auth_msg = SignalingMessage::AuthRequest {
    device_id: args.device_id.clone(),
    password: args.password.clone(),
};
ws_tx.send(Message::Text(serde_json::to_string(&auth_msg)?.into())).await?;

// 等待认证响应
match ws_rx.next().await {
    Some(Ok(Message::Text(text))) => {
        let response: SignalingMessage = serde_json::from_str(&text)?;
        match response {
            SignalingMessage::AuthResponse { success: true, .. } => {
                tracing::info!("认证成功");
            }
            SignalingMessage::AuthResponse { success: false, message } => {
                tracing::error!("认证失败: {:?}", message);
                return Err(anyhow::anyhow!("认证失败"));
            }
            _ => {
                return Err(anyhow::anyhow!("unexpected response"));
            }
        }
    }
    _ => return Err(anyhow::anyhow!("认证响应接收失败")),
}

// 发送连接请求
let conn = serde_json::to_string(&SignalingMessage::Connect {
    target_device_id: args.target_agent.clone(),
})?;
ws_tx.send(Message::Text(conn.into())).await?;
```

- [ ] **Step 3: 编译验证**

```bash
cargo check -p rdp-client
```

Expected: PASS

---

### Task 5: 添加 rand 依赖

**Files:**
- Modify: `agent/Cargo.toml`

- [ ] **Step 1: 添加 rand 依赖**

在 `[dependencies]` 中添加：

```toml
rand = "0.8"
```

- [ ] **Step 2: 编译验证**

```bash
cargo check -p rdp-agent
```

Expected: PASS

---

### Task 6: 最终验证

- [ ] **Step 1: 全量编译检查**

```bash
cargo check --workspace
```

Expected: PASS

- [ ] **Step 2: 运行 agent 测试**

```bash
cargo test -p rdp-agent -- --nocapture
```

Expected: PASS

- [ ] **Step 3: 提交到 git**

```bash
git add -A
git commit -m "feat: add password authentication mechanism

- Add AuthRequest/AuthResponse to signaling protocol
- Create DeviceAuthManager for device password management
- Integrate authentication in agent and client
- Add rand dependency for password generation"
```
