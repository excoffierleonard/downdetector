use log::{error, info, warn};
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tokio::time;

use crate::config::Config;
use crate::error::Error;

/// Monitors websites periodically and prints their status
pub async fn monitor_websites() {
    let config = Config::load().expect("Failed to load configuration");

    info!("Starting website monitoring...");
    info!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    info!("Timeout: {} seconds", config.config.timeout_secs);
    info!("Monitoring {} websites", config.sites.urls.len());

    loop {
        info!("Checking website status...");

        for url in &config.sites.urls {
            if let Err(e) = monitor_website_status(
                url,
                config.config.timeout_secs,
                &config.config.discord_id,
                &config.config.webhook_url,
            )
            .await
            {
                error!("Error checking {}: {}", url, e);
            }
        }

        // Sleep for the configured interval before the next check
        time::sleep(Duration::from_secs(config.config.check_interval_secs)).await;
    }
}

async fn monitor_website_status(
    url: &str,
    timeout_secs: u64,
    discord_id: &Option<u64>,
    webhook_url: &str,
) -> Result<(), Error> {
    match is_url_up(url, timeout_secs).await? {
        true => info!("{}: UP", url),
        false => {
            warn!("{}: DOWN", url);

            let message = format!("Alert: {} is DOWN!", url);
            send_discord_notification(webhook_url, &message, discord_id).await?;
        }
    }
    Ok(())
}

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

async fn send_discord_notification(
    webhook_url: &str,
    message: &str,
    discord_id: &Option<u64>,
) -> Result<(), Error> {
    let client = Client::new();

    let tag = discord_id.map_or(String::new(), |id| format!("<@{}> ", id));

    let payload = DiscordMessage {
        content: format!("{}{}", tag, message).to_string(),
    };

    client.post(webhook_url).json(&payload).send().await?;
    Ok(())
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
        let discord_id: u64 = dotenvy::var("DISCORD_ID")
            .expect("DISCORD_ID not set")
            .parse()
            .expect("Invalid DISCORD_ID");
        let message = "Test notification from Rust!";
        let result = send_discord_notification(&webhook_url, &message, &Some(discord_id)).await;
        assert!(
            result.is_ok(),
            "Expected notification to be sent successfully"
        );
    }
}
