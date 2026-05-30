#[path = "./common.rs"]
mod common;
use common::start_test_server;
use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use rdp_common::signaling::SignalingMessage;

#[tokio::test]
async fn test_signaling_server_connection() {
    let (url, _handle) = start_test_server().await;
    let result = connect_async(&url).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_device_registration() {
    let (url, _handle) = start_test_server().await;
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut tx, mut rx) = ws.split();
    
    let reg = SignalingMessage::Register {
        device_id: "test-device".to_string(),
    };
    tx.send(Message::Text(
        serde_json::to_string(&reg).unwrap().into()
    )).await.unwrap();
    
    let resp = rx.next().await.unwrap().unwrap();
    let msg: SignalingMessage = serde_json::from_str(&resp.to_text().unwrap()).unwrap();
    assert!(matches!(msg, SignalingMessage::Register { .. }));
}

#[tokio::test]
async fn test_offer_answer_exchange() {
    let (url, _handle) = start_test_server().await;
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut tx, mut rx) = ws.split();
    
    let offer = SignalingMessage::Offer {
        sdp: "test-sdp".to_string(),
    };
    tx.send(Message::Text(
        serde_json::to_string(&offer).unwrap().into()
    )).await.unwrap();
    
    let resp = rx.next().await.unwrap().unwrap();
    let msg: SignalingMessage = serde_json::from_str(&resp.to_text().unwrap()).unwrap();
    assert!(matches!(msg, SignalingMessage::Offer { .. }));
}
