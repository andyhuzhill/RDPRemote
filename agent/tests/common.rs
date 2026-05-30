use tokio::net::TcpListener;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use rdp_common::signaling::SignalingMessage;

/// 启动测试用信令服务器
pub async fn start_test_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("ws://{}", addr);
    
    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                let (mut tx, mut rx) = ws.split();
                
                while let Some(Ok(msg)) = rx.next().await {
                    // 回显消息
                    tx.send(msg).await.ok();
                }
            });
        }
    });
    
    (url, handle)
}
