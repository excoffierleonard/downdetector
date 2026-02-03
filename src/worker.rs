use log::{error, info, warn};
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tokio::{select, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::Error;

/// Continuously monitors configured websites and reports their status.
///
/// This function runs indefinitely, checking all configured websites at regular
/// intervals and sending Discord notifications when sites are detected as down.
///
/// # Behavior
///
/// - Loads configuration from the default config file location
/// - Checks each configured URL for availability
/// - Logs the status of each site (UP/DOWN)
/// - Sends Discord webhook notifications for DOWN sites (if configured)
/// - Sleeps for the configured interval before the next check cycle
///
/// # Panics
///
/// Panics if the configuration cannot be loaded at startup.
pub async fn monitor_websites(token: CancellationToken) {
    let config = Config::load().expect("Failed to load configuration");

    // Intial Configuration Logging
    info!("Starting website monitoring...");
    info!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    info!("Timeout: {} seconds", config.config.timeout_secs);
    match (
        config.config.webhook_url.is_some(),
        config.config.discord_id.is_some(),
    ) {
        (true, true) => {
            info!("Webhook is set, a notification will be sent on failure");
            info!("Discord ID is set, notifications will be tagged for the user");
        }
        (true, false) => {
            info!("Webhook is set, a notification will be sent on failure");
            warn!("Discord ID is not set, notifications will not tag any user");
        }
        (false, _) => warn!("Webhook is not set, no notifications will be sent"),
    }
    info!("Monitoring {} websites", config.sites.urls.len());

    // Main monitoring loop
    loop {
        // Check if we should shutdown before starting new cycle
        if token.is_cancelled() {
            info!("Shutdown requested, stopping monitor");
            break;
        }

        info!("Checking website status...");

        for url in &config.sites.urls {
            if let Err(e) = monitor_website_status(
                url,
                config.config.timeout_secs,
                config.config.discord_id.as_ref(),
                config.config.webhook_url.as_ref(),
            )
            .await
            {
                error!("Error checking {url}: {e}");
            }
        }

        // Interruptible sleep
        select! {
            () = sleep(Duration::from_secs(config.config.check_interval_secs)) => {},
            () = token.cancelled() => {
                info!("Shutdown requested during sleep");
                break;
            }
        }
    }

    // Cleanup and shutdown
    info!("Website monitoring stopped gracefully");
}

async fn monitor_website_status(
    url: &str,
    timeout_secs: u64,
    discord_id: Option<&u64>,
    webhook_url: Option<&String>,
) -> Result<(), Error> {
    if is_url_up(url, timeout_secs).await? {
        info!("{url}: UP");
    } else {
        warn!("{url}: DOWN");

        if let Some(webhook) = webhook_url {
            let message = format!("Alert: {url} is DOWN!");
            send_discord_notification(webhook, &message, discord_id).await?;
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
    discord_id: Option<&u64>,
) -> Result<(), Error> {
    let client = Client::new();

    // If discord_id is None, we don't want to mention anyone
    let tag = discord_id.map_or(String::new(), |id| format!("<@{id}> "));

    let payload = DiscordMessage {
        content: format!("{tag}{message}").to_string(),
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
        let result = send_discord_notification(&webhook_url, message, Some(&discord_id)).await;
        assert!(
            result.is_ok(),
            "Expected notification to be sent successfully"
        );
    }
}
