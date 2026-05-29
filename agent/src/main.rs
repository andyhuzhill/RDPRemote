use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    FmtSubscriber::builder().with_max_level(tracing::Level::INFO).init();
    tracing::info!("RDP Agent starting...");
    tracing::info!("Agent will connect to signaling server and provide remote desktop services");

    Ok(())
}
