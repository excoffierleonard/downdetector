#[tokio::main]
async fn main() {
    downdetector::monitor_websites().await;
}
