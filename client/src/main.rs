use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    FmtSubscriber::builder().with_max_level(tracing::Level::INFO).init();
    tracing::info!("RDP Client starting...");
    tracing::info!("Client will connect to signaling server and control remote desktop");

    Ok(())
}
