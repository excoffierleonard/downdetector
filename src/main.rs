use downdetector::{config::Config, monitor_websites};
use std::process;

#[tokio::main]
async fn main() {
    // Load configuration from config.toml file
    let config = match Config::load("config.toml") {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error loading configuration: {}", err);
            process::exit(1);
        }
    };

    println!("Starting website monitoring...");
    println!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    println!("Timeout: {} seconds", config.config.timeout_secs);
    println!("Monitoring {} websites", config.sites.urls.len());

    // Start the monitoring loop
    monitor_websites(
        config.sites.urls,
        config.config.timeout_secs,
        config.config.check_interval_secs,
        config.config.discord_id.unwrap(),
        config.config.webhook_url.unwrap(),
    )
    .await;
}
