#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Create cancellation token
    let token = tokio_util::sync::CancellationToken::new();

    // Spawn the shutdown handler
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
        shutdown_token.cancel();
    });

    // Start monitoring (runs forever)
    downdetector::monitor_websites(token).await;
}
