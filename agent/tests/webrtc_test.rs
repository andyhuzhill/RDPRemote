#[path = "./common.rs"]
mod common;
use common::start_test_server;

#[tokio::test]
async fn test_webrtc_connection() {
    // 测试 WebRTC 连接建立
    let (url, _handle) = start_test_server().await;
    println!("测试服务器地址: {}", url);
    // TODO: 实现 WebRTC 连接测试
}

#[tokio::test]
async fn test_ice_candidate_exchange() {
    // 测试 ICE candidate 交换
    let (url, _handle) = start_test_server().await;
    println!("测试服务器地址: {}", url);
    // TODO: 实现 ICE candidate 交换测试
}

#[tokio::test]
async fn test_data_channel() {
    // 测试数据通道
    let (url, _handle) = start_test_server().await;
    println!("测试服务器地址: {}", url);
    // TODO: 实现数据通道测试
}
