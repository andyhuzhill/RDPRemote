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
    let (mut tx, mut rx) = ws_stream.split();

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
            let _ = tx.send(Message::Text(r#"{"type":"error","message":"invalid register"}"#.into())).await;
            return;
        }
    };

    // 创建通道用于向该设备发送消息
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Message>(100);
    registry.insert(device_id.clone(), msg_tx);
    tracing::info!("Device {} registered", device_id);

    // 启动消息转发任务（预留扩展点）
    tokio::spawn(async move {
        while let Some(_msg) = msg_rx.recv().await {
            // 预留：可以在这里实现更复杂的消息处理逻辑
        }
    });

    // 处理传入的消息
    while let Some(msg_result) = rx.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(_) => break,
        };

        if let Message::Text(text) = &msg {
            if let Some(target_id) = parse_connect(text) {
                // 转发消息到目标设备
                if let Some(target_tx) = registry.get(&target_id) {
                    let _ = target_tx.send(msg.clone()).await;
                }
            }
        }

        // 回显消息（简单测试用）
        let _ = tx.send(msg).await;
    }

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

fn parse_connect(text: &str) -> Option<String> {
    let msg: SignalingMessage = serde_json::from_str(text).ok()?;
    match msg {
        SignalingMessage::Connect { target_device_id } => Some(target_device_id),
        _ => None,
    }
}
