use dashmap::DashMap;
use futures_util::{sink::SinkExt, stream::StreamExt};
use rdp_common::signaling::SignalingMessage;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tracing_subscriber::FmtSubscriber;

type DeviceRegistry = DashMap<String, tokio::sync::mpsc::Sender<Message>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    FmtSubscriber::builder().with_max_level(tracing::Level::INFO).init();

    let registry: Arc<DeviceRegistry> = Arc::new(DashMap::new());
    let addr = "0.0.0.0:8765";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("WebSocket server listening on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = Arc::clone(&registry);
        tokio::spawn(handle_connection(socket, registry));
    }
}

async fn handle_connection(socket: TcpStream, registry: Arc<DeviceRegistry>) {
    let ws_stream = match tokio_tungstenite::accept_async(socket).await {
        Ok(stream) => stream,
        Err(e) => {
            tracing::error!("WebSocket handshake failed: {}", e);
            return;
        }
    };
    let (tx, mut rx) = ws_stream.split();

    // 处理注册消息
    let register_msg = match rx.next().await {
        Some(Ok(Message::Text(text))) => text,
        _ => {
            tracing::warn!("Connection closed before registration");
            return;
        }
    };

    let device_id = match parse_register(&register_msg) {
        Some(id) => id,
        None => {
            let mut tx = tx;
            let _ = tx.send(Message::Text(r#"{"type":"error","message":"invalid register"}"#.into())).await;
            return;
        }
    };

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

