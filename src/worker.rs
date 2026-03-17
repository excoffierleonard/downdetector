use log::{error, info, warn};
use reqwest::Client;
use serde::Serialize;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::{select, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::error::Error;

const FAILED_SITE_RETRY_SECS: u64 = 10;

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
/// - Retries unreachable sites every 10 seconds until they recover
/// - Sleeps until the next site-specific check is due
///
/// # Panics
///
/// Panics if the configuration cannot be loaded at startup.
pub async fn monitor_websites(token: CancellationToken) {
    let config = Config::load().expect("Failed to load configuration");
    let now = Instant::now();
    let mut site_states: HashMap<String, SiteState> = config
        .sites
        .urls
        .iter()
        .cloned()
        .map(|url| (url, SiteState::new(now)))
        .collect();

    // Intial Configuration Logging
    info!("Starting website monitoring...");
    info!(
        "Check interval: {} seconds",
        config.config.check_interval_secs
    );
    info!("Failed site retry interval: {FAILED_SITE_RETRY_SECS} seconds");
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
    'monitor: loop {
        // Check if we should shutdown before starting new cycle
        if token.is_cancelled() {
            info!("Shutdown requested, stopping monitor");
            break;
        }

        let now = Instant::now();
        let due_urls: Vec<&str> = config
            .sites
            .urls
            .iter()
            .filter_map(|url| {
                site_states
                    .get(url.as_str())
                    .filter(|state| state.is_due(now))
                    .map(|_| url.as_str())
            })
            .collect();

        if due_urls.is_empty() {
            let sleep_duration =
                next_sleep_duration(site_states.values(), config.config.check_interval_secs, now);

            select! {
                () = sleep(sleep_duration) => {},
                () = token.cancelled() => {
                    info!("Shutdown requested during sleep");
                    break;
                }
            }

            continue;
        }

        info!("Checking {} website(s)...", due_urls.len());

        for url in due_urls {
            if token.is_cancelled() {
                info!("Shutdown requested, stopping monitor");
                break 'monitor;
            }

            let site_state = site_states
                .get_mut(url)
                .expect("Site state missing for configured URL");

            if let Err(e) = monitor_website_status(
                url,
                config.config.timeout_secs,
                config.config.check_interval_secs,
                config.config.failure_threshold,
                site_state,
                config.config.discord_id.as_ref(),
                config.config.webhook_url.as_ref(),
            )
            .await
            {
                error!("Error checking {url}: {e}");
            }
        }
    }

    // Cleanup and shutdown
    info!("Website monitoring stopped gracefully");
}

#[derive(Debug, Clone, Copy)]
struct SiteState {
    consecutive_failures: u64,
    last_alert_at: Option<Instant>,
    next_check_at: Instant,
}

impl SiteState {
    fn new(now: Instant) -> Self {
        Self {
            consecutive_failures: 0,
            last_alert_at: None,
            next_check_at: now,
        }
    }

    fn is_due(&self, now: Instant) -> bool {
        self.next_check_at <= now
    }

    fn schedule_next_check(&mut self, checked_at: Instant, check_interval_secs: u64) {
        self.next_check_at = checked_at + self.next_check_delay(check_interval_secs);
    }

    fn next_check_delay(&self, check_interval_secs: u64) -> Duration {
        if self.consecutive_failures == 0 {
            Duration::from_secs(check_interval_secs)
        } else {
            Duration::from_secs(FAILED_SITE_RETRY_SECS)
        }
    }
}

async fn monitor_website_status(
    url: &str,
    timeout_secs: u64,
    check_interval_secs: u64,
    failure_threshold: u64,
    site_state: &mut SiteState,
    discord_id: Option<&u64>,
    webhook_url: Option<&String>,
) -> Result<(), Error> {
    let checked_at = Instant::now();
    let is_up = is_url_up(url, timeout_secs).await?;

    let status = record_site_check(
        site_state,
        is_up,
        failure_threshold,
        check_interval_secs,
        checked_at,
    );
    site_state.schedule_next_check(checked_at, check_interval_secs);

    match status {
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
        } => warn!(
            "{url}: UNREACHABLE ({consecutive_failures}/{failure_threshold} consecutive failed checks before alerting; retrying in {FAILED_SITE_RETRY_SECS} seconds)"
        ),
        SiteCheckStatus::Down {
            consecutive_failures,
            should_alert: true,
        } => {
            warn!("{url}: DOWN ({consecutive_failures} consecutive failed checks)");

            if let Some(webhook) = webhook_url {
                let message = format!("Alert: {url} is DOWN!");
                send_discord_notification(webhook, &message, discord_id).await?;
            }
        }
        SiteCheckStatus::Down {
            consecutive_failures,
            should_alert: false,
        } => warn!("{url}: DOWN ({consecutive_failures} consecutive failed checks)"),
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
    },
    Down {
        consecutive_failures: u64,
        should_alert: bool,
    },
}

fn record_site_check(
    site_state: &mut SiteState,
    is_up: bool,
    failure_threshold: u64,
    alert_repeat_interval_secs: u64,
    checked_at: Instant,
) -> SiteCheckStatus {
    if is_up {
        let recovered_after_failures = site_state.consecutive_failures;
        site_state.consecutive_failures = 0;
        site_state.last_alert_at = None;

        return SiteCheckStatus::Up {
            recovered_after_failures,
        };
    }

    site_state.consecutive_failures += 1;

    if site_state.consecutive_failures < failure_threshold {
        return SiteCheckStatus::Unreachable {
            consecutive_failures: site_state.consecutive_failures,
            failure_threshold,
        };
    }

    let should_alert = match site_state.last_alert_at {
        None => true,
        Some(last_alert_at) => {
            checked_at.saturating_duration_since(last_alert_at)
                >= Duration::from_secs(alert_repeat_interval_secs)
        }
    };

    if should_alert {
        site_state.last_alert_at = Some(checked_at);
    }

    SiteCheckStatus::Down {
        consecutive_failures: site_state.consecutive_failures,
        should_alert,
    }
}

fn next_sleep_duration<'a>(
    site_states: impl Iterator<Item = &'a SiteState>,
    check_interval_secs: u64,
    now: Instant,
) -> Duration {
    site_states
        .map(|state| state.next_check_at.saturating_duration_since(now))
        .min()
        .unwrap_or_else(|| Duration::from_secs(check_interval_secs))
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
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn test_failures_do_not_alert_before_threshold() {
        let started_at = Instant::now();
        let mut site_state = SiteState::new(started_at);

        for expected_failures in 1..5 {
            let checked_at =
                started_at + Duration::from_secs((expected_failures - 1) * FAILED_SITE_RETRY_SECS);
            let status = record_site_check(&mut site_state, false, 5, 60, checked_at);
            assert_eq!(
                status,
                SiteCheckStatus::Unreachable {
                    consecutive_failures: expected_failures,
                    failure_threshold: 5,
                }
            );
        }
    }

    #[test]
    fn test_threshold_and_later_failures_alert_are_throttled() {
        let started_at = Instant::now();
        let mut site_state = SiteState::new(started_at);

        for offset_secs in [0, 10, 20, 30] {
            record_site_check(
                &mut site_state,
                false,
                5,
                60,
                started_at + Duration::from_secs(offset_secs),
            );
        }

        let threshold_status = record_site_check(
            &mut site_state,
            false,
            5,
            60,
            started_at + Duration::from_secs(40),
        );
        assert_eq!(
            threshold_status,
            SiteCheckStatus::Down {
                consecutive_failures: 5,
                should_alert: true,
            }
        );

        let later_status = record_site_check(
            &mut site_state,
            false,
            5,
            60,
            started_at + Duration::from_secs(50),
        );
        assert_eq!(
            later_status,
            SiteCheckStatus::Down {
                consecutive_failures: 6,
                should_alert: false,
            }
        );

        let throttled_repeat = record_site_check(
            &mut site_state,
            false,
            5,
            60,
            started_at + Duration::from_secs(100),
        );
        assert_eq!(
            throttled_repeat,
            SiteCheckStatus::Down {
                consecutive_failures: 7,
                should_alert: true,
            }
        );
    }

    #[test]
    fn test_success_resets_failure_count_and_alert_state() {
        let started_at = Instant::now();
        let mut site_state = SiteState::new(started_at);

        for offset_secs in [0, 10, 20, 30, 40] {
            record_site_check(
                &mut site_state,
                false,
                5,
                60,
                started_at + Duration::from_secs(offset_secs),
            );
        }

        let recovered = record_site_check(
            &mut site_state,
            true,
            5,
            60,
            started_at + Duration::from_secs(50),
        );
        assert_eq!(
            recovered,
            SiteCheckStatus::Up {
                recovered_after_failures: 5,
            }
        );
        assert_eq!(site_state.consecutive_failures, 0);
        assert!(site_state.last_alert_at.is_none());

        let next_failure = record_site_check(
            &mut site_state,
            false,
            5,
            60,
            started_at + Duration::from_secs(60),
        );
        assert_eq!(
            next_failure,
            SiteCheckStatus::Unreachable {
                consecutive_failures: 1,
                failure_threshold: 5,
            }
        );
    }

    #[test]
    fn test_failed_sites_are_scheduled_for_fast_retries() {
        let checked_at = Instant::now();
        let mut site_state = SiteState::new(checked_at);

        site_state.schedule_next_check(checked_at, 60);
        assert_eq!(
            site_state.next_check_at.duration_since(checked_at),
            Duration::from_secs(60)
        );

        site_state.consecutive_failures = 1;
        site_state.schedule_next_check(checked_at, 60);
        assert_eq!(
            site_state.next_check_at.duration_since(checked_at),
            Duration::from_secs(FAILED_SITE_RETRY_SECS)
        );
    }

    #[test]
    fn test_next_sleep_duration_uses_earliest_site_check() {
        let now = Instant::now();
        let healthy_site = SiteState {
            consecutive_failures: 0,
            last_alert_at: None,
            next_check_at: now + Duration::from_secs(60),
        };
        let failing_site = SiteState {
            consecutive_failures: 1,
            last_alert_at: None,
            next_check_at: now + Duration::from_secs(FAILED_SITE_RETRY_SECS),
        };

        let sleep_duration =
            next_sleep_duration([&healthy_site, &failing_site].into_iter(), 60, now);

        assert_eq!(sleep_duration, Duration::from_secs(FAILED_SITE_RETRY_SECS));
    }

    #[tokio::test]
    async fn test_local_success_url_is_up() {
        let url = spawn_test_http_server(
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
        )
        .await;

        let result = is_url_up(&url, 5).await.unwrap();
        assert!(result, "Expected local test server to be up");
    }

    #[tokio::test]
    async fn test_local_non_success_url_is_down() {
        let url = spawn_test_http_server(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\nConnection: close\r\n\r\nDOWN",
        )
        .await;

        let result = is_url_up(&url, 5).await.unwrap();
        assert!(!result, "Expected local test server to be reported as down");
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

    async fn spawn_test_http_server(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind local test server");
        let addr = listener
            .local_addr()
            .expect("Failed to read local test server address");

        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("Failed to accept local test connection");
            let mut request_buf = [0_u8; 1024];
            let _ = stream.read(&mut request_buf).await;
            stream
                .write_all(response.as_bytes())
                .await
                .expect("Failed to write local test response");
            stream
                .shutdown()
                .await
                .expect("Failed to close local test response stream");
        });

        format!("http://{addr}")
    }
}
