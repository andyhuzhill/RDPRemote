mod auth;

use dashmap::DashMap;
use futures_util::{sink::SinkExt, stream::StreamExt};
use rdp_common::signaling::SignalingMessage;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tracing_subscriber::FmtSubscriber;

use auth::AuthManager;

type DeviceRegistry = DashMap<String, tokio::sync::mpsc::Sender<Message>>;

/// 从环境变量获取 JWT 密钥，默认使用开发密钥
fn get_jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "rdp-dev-secret-key-change-in-production".to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    FmtSubscriber::builder().with_max_level(tracing::Level::INFO).init();

    let registry: Arc<DeviceRegistry> = Arc::new(DashMap::new());
    let auth = Arc::new(AuthManager::new(get_jwt_secret()));
    let addr = "0.0.0.0:8765";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("WebSocket server listening on {}", addr);
    tracing::info!("JWT authentication enabled");

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = Arc::clone(&registry);
        let auth = Arc::clone(&auth);
        tokio::spawn(handle_connection(socket, registry, auth));
    }
}

async fn handle_connection(
    socket: TcpStream,
    registry: Arc<DeviceRegistry>,
    auth: Arc<AuthManager>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(socket).await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::error!("WebSocket handshake failed: {}", e);
            return;
        }
    };
    let (tx, mut rx) = ws_stream.split();

    // 第一步：认证
    let auth_msg = match rx.next().await {
        Some(Ok(Message::Text(text))) => text,
        _ => {
            tracing::warn!("Connection closed before authentication");
            let mut tx = tx;
            let _ = tx
                .send(Message::Text(
                    r#"{"type":"auth-response","success":false,"message":"no auth token"}"#.into(),
                ))
                .await;
            return;
        }
    };

    let (device_id, tx) = match handle_auth(&auth_msg, &auth, tx).await {
        Some((id, tx)) => (id, tx),
        None => {
            return;
        }
    };

    // 第二步：注册（认证后）
    let register_msg = match rx.next().await {
        Some(Ok(Message::Text(text))) => text,
        _ => {
            tracing::warn!("Connection closed before registration");
            return;
        }
    };

    let registered_device_id = match parse_register(&register_msg) {
        Some(id) => id,
        None => {
            let mut tx = tx;
            let _ = tx
                .send(Message::Text(
                    r#"{"type":"error","message":"invalid register"}"#.into(),
                ))
                .await;
            return;
        }
    };

    // 验证注册的设备 ID 与认证的设备 ID 一致
    if registered_device_id != device_id {
        tracing::warn!(
            "Device ID mismatch: auth={} register={}",
            device_id,
            registered_device_id
        );
        let mut tx = tx;
        let _ = tx
            .send(Message::Text(
                r#"{"type":"auth-response","success":false,"message":"device id mismatch"}"#.into(),
            ))
            .await;
        return;
    }

    // 创建通道用于向该设备发送消息
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(100);
    registry.insert(device_id.clone(), msg_tx);
    tracing::info!("Device {} registered", device_id);

    // 启动发送任务：将来自通道的消息发送到 WebSocket
    let mut tx = tx;
    let send_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // 处理接收到的消息
    while let Some(msg_result) = rx.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(_) => break,
        };

        if let Message::Text(text) = &msg {
            match serde_json::from_str::<SignalingMessage>(text) {
                Ok(SignalingMessage::Connect { target_device_id }) => {
                    // 转发 Connect 消息到目标设备
                    if let Some(target_tx) = registry.get(&target_device_id) {
                        let connect_msg = SignalingMessage::Connect {
                            target_device_id: device_id.clone(),
                        };
                        match serde_json::to_string(&connect_msg) {
                            Ok(json) => {
                                if let Err(e) = target_tx.send(Message::Text(json.into())).await {
                                    tracing::warn!("Failed to send connect to {}: {}", target_device_id, e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize connect message: {}", e);
                            }
                        }
                    } else {
                        tracing::warn!("Target device {} not found for connect", target_device_id);
                    }
                }
                Ok(SignalingMessage::Offer { .. })
                | Ok(SignalingMessage::Answer { .. })
                | Ok(SignalingMessage::IceCandidate { .. }) => {
                    // 广播信令消息给其他设备
                    for entry in registry.iter() {
                        if entry.key() != &device_id {
                            let _ = entry.value().send(msg.clone()).await;
                        }
                    }
                }
                _ => {
                    // 忽略其他消息类型
                }
            }
        }
    }

    send_task.abort();
    registry.remove(&device_id);
    tracing::info!("Device {} disconnected", device_id);
}

fn parse_register(text: &str) -> Option<String> {
    let msg: SignalingMessage = serde_json::from_str(text).ok()?;
    match msg {
        SignalingMessage::Register { device_id } => Some(device_id),
        _ => None,
    }
}

/// 处理认证消息
async fn handle_auth(
    text: &str,
    auth: &AuthManager,
    mut tx: futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, Message>,
) -> Option<(String, futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, Message>)> {
    let msg: SignalingMessage = serde_json::from_str(text).ok()?;

    match msg {
        SignalingMessage::Auth { token } => {
            match auth.verify_token(&token) {
                Ok(device_id) => {
                    tracing::info!("Device {} authenticated", device_id);
                    let response = SignalingMessage::AuthResponse {
                        success: true,
                        message: None,
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = tx.send(Message::Text(json.into())).await;
                    }
                    Some((device_id, tx))
                }
                Err(e) => {
                    tracing::warn!("Token verification failed: {}", e);
                    let response = SignalingMessage::AuthResponse {
                        success: false,
                        message: Some("invalid token".to_string()),
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = tx.send(Message::Text(json.into())).await;
                    }
                    None
                }
            }
        }
        _ => {
            tracing::warn!("Expected auth message, got {:?}", msg);
            let response = SignalingMessage::AuthResponse {
                success: false,
                message: Some("expected auth message".to_string()),
            };
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = tx.send(Message::Text(json.into())).await;
            }
            None
        }
    }
}

