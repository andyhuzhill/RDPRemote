//! RDP Client - 远程桌面控制客户端

use anyhow::{Context, Result};
use clap::Parser;
use futures_util::{sink::SinkExt, stream::StreamExt};
use rdp_client::webrtc::ClientPeer;
use rdp_common::signaling::SignalingMessage;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "ws://localhost:8765")]
    server: String,

    #[arg(short, long, default_value = "client-1")]
    device_id: String,

    #[arg(short, long, required = true)]
    target_agent: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    tracing::info!("RDP Client starting... Device ID: {}", args.device_id);

    run_client(args).await
}

async fn run_client(args: Args) -> Result<()> {
    // 初始化 WebRTC peer
    let mut peer = ClientPeer::new().await.context("Failed to create WebRTC peer")?;

    // 创建消息通道，用于从 ICE 回调转发消息到主循环
    let (msg_tx, mut msg_rx) = mpsc::channel::<SignalingMessage>(100);

    // 连接信令服务器
    let (ws_stream, _) = connect_async(&args.server).await
        .context("Failed to connect to signaling server")?;
    let (ws_tx, ws_rx) = ws_stream.split();
    let mut ws_tx = ws_tx;
    let mut ws_rx = ws_rx;

    // 注册设备
    let reg = serde_json::to_string(&SignalingMessage::Register {
        device_id: args.device_id.clone(),
    })?;
    ws_tx.send(Message::Text(reg.into())).await?;

    // 发送连接请求
    let conn = serde_json::to_string(&SignalingMessage::Connect {
        target_device_id: args.target_agent.clone(),
    })?;
    ws_tx.send(Message::Text(conn.into())).await?;
    tracing::info!("Sent connect request to: {}", args.target_agent);

    // 注册 ICE 候选收集回调，自动转发给对端
    let ice_tx = msg_tx.clone();
    peer.on_ice_candidate(move |candidate| {
        let tx = ice_tx.clone();
        Box::pin(async move {
            if let Ok(ice_init) = candidate.to_json() {
                let ice_msg = SignalingMessage::IceCandidate {
                    candidate: ice_init.candidate,
                    sdp_mid: ice_init.sdp_mid.unwrap_or_default(),
                    sdp_m_line_index: ice_init.sdp_mline_index.unwrap_or(0),
                };
                if let Err(e) = tx.send(ice_msg).await {
                    tracing::warn!("Failed to send ICE candidate: {}", e);
                }
            }
        })
    });

    // 信令循环：等待 Offer，发送 Answer
    tracing::info!("Waiting for signaling messages...");
    let mut webrtc_ready = false;

    while !webrtc_ready {
        tokio::select! {
            // 处理来自 WebSocket 的信令消息
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<SignalingMessage>(&text) {
                            Ok(SignalingMessage::Offer { sdp }) => {
                                tracing::info!("Received offer");
                                peer.set_offer(&sdp).await?;
                                peer.add_video_transceiver().await?;

                                let answer = peer.create_answer().await?;
                                peer.set_local_description(&answer).await?;

                                let msg = serde_json::to_string(&SignalingMessage::Answer { sdp: answer })?;
                                ws_tx.send(Message::Text(msg.into())).await?;
                                tracing::info!("Sent answer");

                                peer.start_receiving_video();
                                webrtc_ready = true;
                            }
                            Ok(SignalingMessage::IceCandidate { candidate, sdp_mid, sdp_m_line_index }) => {
                                peer.add_ice_candidate(&candidate, sdp_m_line_index, &sdp_mid).await?;
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => return Err(anyhow::anyhow!("WebSocket error: {}", e)),
                    None => return Err(anyhow::anyhow!("WebSocket closed")),
                    _ => {}
                }
            }
            // 处理从 ICE 回调发来的消息
            Some(ice_msg) = msg_rx.recv() => {
                let msg = serde_json::to_string(&ice_msg)?;
                ws_tx.send(Message::Text(msg.into())).await?;
            }
        }
    }

    // 等待 WebRTC 连接
    tracing::info!("Waiting for WebRTC connection...");
    for _ in 0..100 {
        let state = peer.connection_state();
        use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState::*;
        match state {
            Connected => {
                tracing::info!("WebRTC connected!");
                break;
            }
            Failed | Closed | Disconnected => {
                return Err(anyhow::anyhow!("WebRTC connection failed: {:?}", state));
            }
            _ => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    // Phase 1a: 简单的帧接收循环
    // 后续集成 wgpu 渲染器
    tracing::info!("Waiting for video frames...");
    let mut video_rx = peer.take_video_receiver()
        .ok_or_else(|| anyhow::anyhow!("Video receiver already taken"))?;

    while let Some(frame) = video_rx.recv().await {
        tracing::debug!("Received frame: {} bytes, keyframe={}", frame.data.len(), frame.is_keyframe);
        // TODO: 解码 VP9 并渲染
    }

    tracing::info!("Client exiting");
    Ok(())
}
