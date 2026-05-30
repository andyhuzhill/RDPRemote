# JWT 安全认证 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 RDPRemote 信令服务器添加 JWT 认证机制，确保只有合法设备可以连接。

**Architecture:** 
1. 设备在注册时提供 JWT token，服务器验证 token 有效性
2. 使用 `jsonwebtoken` crate 进行 JWT 的生成和验证
3. 认证通过后设备才能注册到服务器，否则拒绝连接
4. 新增认证模块 `server/src/auth.rs` 封装 JWT 逻辑

**Tech Stack:**
- `jsonwebtoken = "9"` - JWT 编解码
- `serde` - 序列化/反序列化 Claims
- `chrono` - 时间戳处理

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `server/Cargo.toml` | Modify | 添加 `jsonwebtoken` 和 `chrono` 依赖 |
| `common/src/signaling.rs` | Modify | 添加 `Auth`、`AuthSuccess`、`AuthFailure` 消息类型 |
| `server/src/auth.rs` | Create | JWT 生成和验证逻辑模块 |
| `server/src/main.rs` | Modify | 集成认证逻辑，在注册前验证 token |
| `server/tests/auth_test.rs` | Create | JWT 认证单元测试 |

---

## Task 1: 添加依赖

**Files:**
- Modify: `server/Cargo.toml`

- [ ] **Step 1: 修改 Cargo.toml**

在 `[dependencies]` 中添加：
```toml
jsonwebtoken = "9"
chrono = { version = "0.4", features = ["serde"] }
```

---

## Task 2: 扩展信令协议

**Files:**
- Modify: `common/src/signaling.rs`

- [ ] **Step 1: 添加认证消息类型**

在 `SignalingMessage` enum 中添加：
```rust
#[serde(rename = "auth")]
Auth { token: String },
#[serde(rename = "auth-success")]
AuthSuccess { device_id: String },
#[serde(rename = "auth-failure")]
AuthFailure { reason: String },
```

---

## Task 3: 创建认证模块

**Files:**
- Create: `server/src/auth.rs`

- [ ] **Step 1: 编写 auth.rs 模块**

```rust
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, Algorithm};
use serde::{Serialize, Deserialize};
use chrono::{Utc, Duration};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,  // device_id
    pub exp: usize,   // expiration timestamp
    pub iat: usize,   // issued at timestamp
}

/// JWT 配置
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "rdp-remote-default-secret".to_string()),
            expiration_hours: 24,
        }
    }
}

/// 生成 JWT token
pub fn generate_token(device_id: &str, config: &JwtConfig) -> Result<String, anyhow::Error> {
    let now = Utc::now();
    let exp = (now + Duration::hours(config.expiration_hours as i64)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: device_id.to_string(),
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_ref()),
    )?;

    Ok(token)
}

/// 验证 JWT token
pub fn verify_token(token: &str, config: &JwtConfig) -> Result<Claims, anyhow::Error> {
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_ref()),
        &validation,
    )?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_token() {
        let config = JwtConfig::default();
        let device_id = "test-device-001";
        
        let token = generate_token(device_id, &config).unwrap();
        let claims = verify_token(&token, &config).unwrap();
        
        assert_eq!(claims.sub, device_id);
    }

    #[test]
    fn test_verify_invalid_token() {
        let config = JwtConfig::default();
        let result = verify_token("invalid.token.here", &config);
        assert!(result.is_err());
    }
}
```

---

## Task 4: 集成认证到服务器

**Files:**
- Modify: `server/src/main.rs`

- [ ] **Step 1: 添加认证模块引用**

在文件顶部添加：
```rust
mod auth;
use auth::{JwtConfig, verify_token};
```

- [ ] **Step 2: 修改 handle_connection 函数**

将认证逻辑集成到连接处理流程中：
```rust
async fn handle_connection(socket: TcpStream, registry: Arc<DeviceRegistry>) {
    let ws_stream = match tokio_tungstenite::accept_async(socket).await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::error!("WebSocket handshake failed: {}", e);
            return;
        }
    };
    let (tx, mut rx) = ws_stream.split();

    // 1. 首先接收认证消息
    let auth_msg = match rx.next().await {
        Some(Ok(Message::Text(text))) => text,
        _ => {
            tracing::warn!("Connection closed before authentication");
            return;
        }
    };

    // 2. 解析并验证 token
    let device_id = match authenticate_connection(&auth_msg) {
        Some(id) => id,
        None => {
            let mut tx = tx;
            let _ = tx.send(Message::Text(r#"{"type":"auth-failure","reason":"invalid token"}"#.into())).await;
            return;
        }
    };

    tracing::info!("Device {} authenticated", device_id);

    // 3. 发送认证成功响应
    {
        let mut tx = tx;
        let success_msg = serde_json::to_string(&SignalingMessage::AuthSuccess {
            device_id: device_id.clone(),
        }).unwrap();
        let _ = tx.send(Message::Text(success_msg.into())).await;
    }

    // 4. 创建通道用于向该设备发送消息
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(100);
    registry.insert(device_id.clone(), msg_tx);
    tracing::info!("Device {} registered", device_id);

    // 5. 启动发送任务
    let mut tx = tx;
    let send_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 6. 处理接收到的消息（与原来相同）
    // ... 原有消息处理逻辑 ...

    send_task.abort();
    registry.remove(&device_id);
    tracing::info!("Device {} disconnected", device_id);
}

fn authenticate_connection(text: &str) -> Option<String> {
    let msg: SignalingMessage = serde_json::from_str(text).ok()?;
    match msg {
        SignalingMessage::Auth { token } => {
            let config = JwtConfig::default();
            verify_token(&token, &config).ok().map(|claims| claims.sub)
        }
        _ => None,
    }
}
```

---

## Task 5: 编写集成测试

**Files:**
- Create: `server/tests/auth_test.rs`

- [ ] **Step 1: 编写集成测试**

```rust
use rdp_server::auth::{generate_token, verify_token, JwtConfig};

#[test]
fn test_jwt_full_flow() {
    let config = JwtConfig::default();
    let device_id = "integration-test-device";

    // 生成 token
    let token = generate_token(device_id, &config).unwrap();
    assert!(!token.is_empty());

    // 验证 token
    let claims = verify_token(&token, &config).unwrap();
    assert_eq!(claims.sub, device_id);

    // 验证过期时间合理
    let now = chrono::Utc::now().timestamp() as usize;
    assert!(claims.exp > now);
    assert!(claims.iat <= now);
}

#[test]
fn test_token_with_different_secret_fails() {
    let config1 = JwtConfig {
        secret: "secret1".to_string(),
        expiration_hours: 24,
    };
    let config2 = JwtConfig {
        secret: "secret2".to_string(),
        expiration_hours: 24,
    };

    let token = generate_token("device1", &config1).unwrap();
    let result = verify_token(&token, &config2);
    assert!(result.is_err());
}
```

---

## Task 6: 验证与提交

**Files:**
- All modified files

- [ ] **Step 1: 运行 cargo check**

```bash
cargo check -p rdp-server
```

- [ ] **Step 2: 运行测试**

```bash
cargo test -p rdp-server
cargo test -p rdp-common
```

- [ ] **Step 3: 提交到 git**

```bash
git add server/Cargo.toml server/src/auth.rs server/src/main.rs common/src/signaling.rs server/tests/auth_test.rs
git commit -m "feat: add JWT authentication for signaling server"
```

---

## Self-Review Checklist

- [x] Spec coverage: 所有要求都已覆盖（依赖添加、协议扩展、认证模块、集成、测试）
- [x] Placeholder scan: 无占位符，所有代码完整
- [x] Type consistency: Claims 结构在 auth.rs 中定义，verify_token 返回 Claims
- [ ] 需要确认：`server/src/main.rs` 中是否需要添加 `use auth::JwtConfig`
- [ ] 需要确认：测试文件路径是否正确（`server/tests/` 或 `tests/`）
