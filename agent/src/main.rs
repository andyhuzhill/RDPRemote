//! RDP Agent - 远程桌面控制代理

use anyhow::{Context, Result};
use clap::Parser;
use futures_util::{sink::SinkExt, stream::StreamExt};
use rdp_common::signaling::SignalingMessage;
use tokio_tungstenite::{connect_async, tungstenite::Message};

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

    // 连接信令服务器
    let (ws_stream, _) = connect_async(&args.server).await
        .context("Failed to connect to signaling server")?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

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

    // 等待 Connect 或 Offer
    let peer = AgentPeer::new().await.context("Failed to create WebRTC peer")?;

    // 信令循环
    tracing::info!("Waiting for signaling messages...");
    let mut connected = false;

    while !connected {
        match ws_rx.next().await {
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

    // 视频流循环
    tracing::info!("WebRTC connected, starting video stream...");
    let mut frame_count = 0u64;
    let frame_interval = std::time::Duration::from_millis(33);

    loop {
        let start = std::time::Instant::now();

        match capture.capture_frame() {
            Ok(frame) => {
                match encoder.encode(&frame.data, frame.width, frame.height, frame.timestamp_us) {
                    Ok(encoded) => {
                        let _ = peer.send_video_frame(encoded.data, 33_333, encoded.is_keyframe).await;
                    }
                    Err(e) => tracing::warn!("Encode error: {}", e),
                }
            }
            Err(e) => tracing::trace!("Capture error: {}", e),
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
