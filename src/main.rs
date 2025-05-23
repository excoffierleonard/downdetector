#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Start monitoring (runs forever)
    downdetector::monitor_websites().await;
}
