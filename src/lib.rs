use reqwest::Client;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;
use tokio::time;

mod config;
mod error;

use crate::config::Config;
use crate::error::Error;

/// Asynchronously checks if a given URL is up (returns a 2xx status).
async fn is_url_up(url: &str, timeout_secs: u64) -> Result<bool, Error> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?;

    Ok(client
        .get(url)
        .send()
        .await
        .map(|resp| resp.status().is_success())
        // We unwrap here since we have no way of distinguishing between a network error and a real down on the server side
        .unwrap_or(false))
}

#[derive(Serialize)]
struct DiscordMessage {
    content: String,
}

async fn send_discord_notification(webhook_url: &str, message: &str) -> Result<(), Error> {
    let client = Client::new();
    let payload = DiscordMessage {
        content: message.to_string(),
    };

    client.post(webhook_url).json(&payload).send().await?;
    Ok(())
}

/// Monitors websites periodically and prints their status
pub async fn monitor_websites(config_path: &str) {
    let config = Config::load(Path::new(config_path)).expect("Failed to load configuration");

    println!("Starting website monitoring...");
    println!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    println!("Timeout: {} seconds", config.config.timeout_secs);
    println!("Monitoring {} websites", config.sites.urls.len());

    loop {
        println!("Checking website status...");

        for url in &config.sites.urls {
            // Need better error handling here
            let status = is_url_up(url, config.config.timeout_secs).await.unwrap();
            let status_text = if status { "UP" } else { "DOWN" };
            println!("{}: {}", url, status_text);

            if !status {
                let message = format!("<@{}> Alert: {} is DOWN!", &config.config.discord_id, url);

                if let Err(err) =
                    send_discord_notification(&config.config.webhook_url, &message).await
                {
                    eprintln!("Failed to send Discord notification: {}", err);
                }
            }
        }

        // Sleep for the configured interval before the next check
        time::sleep(Duration::from_secs(config.config.check_interval_secs)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_google_is_up() {
        let result = is_url_up("https://www.google.com", 5).await.unwrap();
        assert!(result, "Expected Google to be up");
    }

    #[tokio::test]
    async fn test_nonexistent_url_is_down() {
        let result = is_url_up("http://nonexistent.subdomain.rust-lang.org", 5)
            .await
            .unwrap();
        assert!(!result, "Expected nonexistent URL to be down");
    }

    #[ignore = "This test requires a valid Discord webhook URL and ID"]
    #[tokio::test]
    async fn test_discord_notification() {
        let webhook_url = dotenvy::var("WEBHOOK_URL").expect("WEBHOOK_URL not set");
        let discord_id = dotenvy::var("DISCORD_ID").expect("DISCORD_ID not set");
        let message = format!("<@{}> Test notification from Rust!", discord_id);
        let result = send_discord_notification(&webhook_url, &message).await;
        assert!(
            result.is_ok(),
            "Expected notification to be sent successfully"
        );
    }
}
