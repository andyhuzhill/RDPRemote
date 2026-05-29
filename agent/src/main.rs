//! RDP Agent - 远程桌面控制代理

use anyhow::Result;
use clap::Parser;
use rdp_common::signaling::SignalingMessage;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[cfg(target_os = "windows")]
use rdp_agent::adaptive::{AdaptiveController, BandwidthTier};
#[cfg(target_os = "windows")]
use rdp_agent::encoder::VP9Encoder;
#[cfg(target_os = "windows")]
use rdp_agent::screen::{D3D11ScreenCapture, ScreenCapture};
#[cfg(target_os = "windows")]
use rdp_agent::webrtc::AgentPeer;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "ws://localhost:8765")]
    server: String,

    #[arg(short, long, default_value = "agent-1")]
    device_id: String,

    #[arg(short, long)]
    target_device: Option<String>,

    #[arg(short, long, default_value = "2000")]
    bitrate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    tracing::info!("RDP Agent starting... Device ID: {}", args.device_id);

    #[cfg(not(target_os = "windows"))]
    {
        tracing::error!("Screen capture requires Windows. This agent cannot run on this platform.");
        return Err(anyhow::anyhow!("Unsupported platform"));
    }

    #[cfg(target_os = "windows")]
    {
        run_agent(args).await
    }
}

#[cfg(target_os = "windows")]
async fn run_agent(args: Args) -> Result<()> {
    // 初始化屏幕捕获
    let mut capture = D3D11ScreenCapture::new().context("Failed to init screen capture")?;
    let (width, height) = capture.get_dimensions();
    tracing::info!("Screen: {}x{}", width, height);

    // 初始化编码器
    let mut encoder = VP9Encoder::new(width, height, args.bitrate)
        .context("Failed to create VP9 encoder")?;

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
    tracing::info!("Registered as: {}", args.device_id);

    // 如果指定了目标，主动发起连接
    if let Some(ref target) = args.target_device {
        let conn = serde_json::to_string(&SignalingMessage::Connect {
            target_device_id: target.clone(),
        })?;
        ws_tx.send(Message::Text(conn.into())).await?;
    }

    // 创建 WebRTC peer
    let peer = AgentPeer::new().await.context("Failed to create WebRTC peer")?;

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

    // 等待 Connect 或 Offer
    tracing::info!("Waiting for signaling messages...");
    let mut connected = false;

    while !connected {
        tokio::select! {
            // 处理来自 WebSocket 的信令消息
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<SignalingMessage>(&text) {
                            Ok(SignalingMessage::Connect { target_device_id }) => {
                                tracing::info!("Connect from: {}", target_device_id);
                                // 创建并发送 Offer
                                let offer = peer.create_offer().await?;
                                let msg = serde_json::to_string(&SignalingMessage::Offer { sdp: offer })?;
                                ws_tx.send(Message::Text(msg.into())).await?;
                            }
                            Ok(SignalingMessage::Answer { sdp }) => {
                                tracing::info!("Received answer");
                                peer.set_answer(sdp).await?;
                                connected = true;
                            }
                            Ok(SignalingMessage::IceCandidate { candidate, sdp_mid, sdp_m_line_index }) => {
                                peer.add_ice_candidate(candidate, sdp_mid, sdp_m_line_index).await?;
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

    // 视频流循环
    tracing::info!("WebRTC connected, starting video stream...");
    let mut adaptive = AdaptiveController::new();
    let mut frame_count = 0u64;
    let mut frame_interval = std::time::Duration::from_millis(33);

    loop {
        let start = std::time::Instant::now();

        // 获取当前带宽层级并调整帧间隔
        let tier = adaptive.current_tier();
        frame_interval = tier.frame_interval();

        match capture.capture_frame() {
            Ok(frame) => {
                match encoder.encode(&frame.data, frame.width, frame.height, frame.timestamp_us) {
                    Ok(encoded) => {
                        let _ = peer.send_video_frame(encoded.data, 33_333, encoded.is_keyframe).await;
                        adaptive.add_bytes_sent(encoded.data.len() as u64);
                    }
                    Err(e) => tracing::warn!("Encode error: {}", e),
                }
            }
            Err(e) => tracing::trace!("Capture error: {}", e),
        }

        // 检查并调整带宽层级
        if let Some(new_tier) = adaptive.check_and_adjust() {
            tracing::info!("Bandwidth tier changed: {:?}", new_tier);
            // 动态调整编码器参数
            // let _ = encoder.set_bitrate(new_tier.bitrate_kbps());
        }

        frame_count += 1;
        if frame_count % 30 == 0 {
            tracing::info!("Frame {}", frame_count);
        }

        let elapsed = start.elapsed();
        if elapsed < frame_interval {
            tokio::time::sleep(frame_interval - elapsed).await;
        }
    }
}
