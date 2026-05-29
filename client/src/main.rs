use rdp_client::webrtc::ClientPeer;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    FmtSubscriber::builder().with_max_level(tracing::Level::INFO).init();
    tracing::info!("RDP Client starting...");
    tracing::info!("Client will connect to signaling server and control remote desktop");

    // Initialize WebRTC peer
    let _peer = ClientPeer::new().await?;
    tracing::info!("WebRTC peer created successfully");

    // Example: Create answer for incoming offer
    // In production, this would be called after receiving offer from signaling server
    // let answer_sdp = peer.create_answer().await?;
    // tracing::info!("Created answer SDP: {}", answer_sdp);

    Ok(())
}
