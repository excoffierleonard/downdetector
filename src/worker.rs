use log::{error, info, warn};
use reqwest::Client;
use serde::Serialize;
use std::{collections::HashMap, time::Duration};
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
/// - Tracks consecutive failures per site to reduce false positives
/// - Logs the status of each site (UP/UNREACHABLE/DOWN)
/// - Sends Discord webhook notifications for sites that stay down long enough
/// - Sleeps for the configured interval before the next check cycle
///
/// # Panics
///
/// Panics if the configuration cannot be loaded at startup.
pub async fn monitor_websites(token: CancellationToken) {
    let config = Config::load().expect("Failed to load configuration");
    let mut failure_counts = HashMap::with_capacity(config.sites.urls.len());

    // Intial Configuration Logging
    info!("Starting website monitoring...");
    info!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    info!("Timeout: {} seconds", config.config.timeout_secs);
    info!(
        "Failure threshold: {} consecutive failed checks",
        config.config.failure_threshold
    );
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
                config.config.failure_threshold,
                &mut failure_counts,
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
    failure_threshold: u64,
    failure_counts: &mut HashMap<String, u64>,
    discord_id: Option<&u64>,
    webhook_url: Option<&String>,
) -> Result<(), Error> {
    let is_up = is_url_up(url, timeout_secs).await?;

    match record_site_check(failure_counts, url, is_up, failure_threshold) {
        SiteCheckStatus::Up {
            recovered_after_failures: 0,
        } => info!("{url}: UP"),
        SiteCheckStatus::Up {
            recovered_after_failures,
        } => info!(
            "{url}: UP (recovered after {recovered_after_failures} consecutive failed checks)"
        ),
        SiteCheckStatus::Unreachable {
            consecutive_failures,
            failure_threshold,
            should_alert: false,
        } => warn!(
            "{url}: UNREACHABLE ({consecutive_failures}/{failure_threshold} consecutive failed checks before alerting)"
        ),
        SiteCheckStatus::Unreachable {
            consecutive_failures,
            should_alert: true,
            ..
        } => {
            warn!("{url}: DOWN ({consecutive_failures} consecutive failed checks)");

            if let Some(webhook) = webhook_url {
                let message = format!("Alert: {url} is DOWN!");
                send_discord_notification(webhook, &message, discord_id).await?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiteCheckStatus {
    Up {
        recovered_after_failures: u64,
    },
    Unreachable {
        consecutive_failures: u64,
        failure_threshold: u64,
        should_alert: bool,
    },
}

fn record_site_check(
    failure_counts: &mut HashMap<String, u64>,
    url: &str,
    is_up: bool,
    failure_threshold: u64,
) -> SiteCheckStatus {
    if is_up {
        return SiteCheckStatus::Up {
            recovered_after_failures: failure_counts.remove(url).unwrap_or(0),
        };
    }

    let consecutive_failures = failure_counts
        .entry(url.to_string())
        .and_modify(|count| *count += 1)
        .or_insert(1);

    SiteCheckStatus::Unreachable {
        consecutive_failures: *consecutive_failures,
        failure_threshold,
        should_alert: *consecutive_failures >= failure_threshold,
    }
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
    use std::collections::HashMap;

    #[test]
    fn test_failures_do_not_alert_before_threshold() {
        let url = "https://example.com";
        let mut failure_counts = HashMap::new();

        for expected_failures in 1..5 {
            let status = record_site_check(&mut failure_counts, url, false, 5);
            assert_eq!(
                status,
                SiteCheckStatus::Unreachable {
                    consecutive_failures: expected_failures,
                    failure_threshold: 5,
                    should_alert: false,
                }
            );
        }
    }

    #[test]
    fn test_threshold_and_later_failures_alert() {
        let url = "https://example.com";
        let mut failure_counts = HashMap::new();

        for _ in 0..4 {
            record_site_check(&mut failure_counts, url, false, 5);
        }

        let threshold_status = record_site_check(&mut failure_counts, url, false, 5);
        assert_eq!(
            threshold_status,
            SiteCheckStatus::Unreachable {
                consecutive_failures: 5,
                failure_threshold: 5,
                should_alert: true,
            }
        );

        let later_status = record_site_check(&mut failure_counts, url, false, 5);
        assert_eq!(
            later_status,
            SiteCheckStatus::Unreachable {
                consecutive_failures: 6,
                failure_threshold: 5,
                should_alert: true,
            }
        );
    }

    #[test]
    fn test_success_resets_failure_count() {
        let url = "https://example.com";
        let mut failure_counts = HashMap::new();

        for _ in 0..3 {
            record_site_check(&mut failure_counts, url, false, 5);
        }

        let recovered = record_site_check(&mut failure_counts, url, true, 5);
        assert_eq!(
            recovered,
            SiteCheckStatus::Up {
                recovered_after_failures: 3,
            }
        );

        let next_failure = record_site_check(&mut failure_counts, url, false, 5);
        assert_eq!(
            next_failure,
            SiteCheckStatus::Unreachable {
                consecutive_failures: 1,
                failure_threshold: 5,
                should_alert: false,
            }
        );
    }

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
