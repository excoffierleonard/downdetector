use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tokio::time;

mod config;

use crate::config::Config;

/// Asynchronously checks if a given URL is up (returns a 2xx status).
async fn is_url_up(url: &str, timeout_secs: u64) -> bool {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap();

    client
        .get(url)
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}

#[derive(Serialize)]
struct DiscordMessage {
    content: String,
}

async fn send_discord_notification(webhook_url: &str, message: &str) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let payload = DiscordMessage {
        content: message.to_string(),
    };

    client.post(webhook_url).json(&payload).send().await?;
    Ok(())
}

/// Monitors websites periodically and prints their status
pub async fn monitor_websites(config_path: &str) {
    let config = Config::load(config_path).expect("Failed to load configuration");

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
            let status = is_url_up(url, config.config.timeout_secs).await;
            let status_text = if status { "UP" } else { "DOWN" };
            println!("{}: {}", url, status_text);

            if !status {
                let message = format!(
                    "<@{}> Alert: {} is DOWN!",
                    config.config.discord_id.as_ref().unwrap(),
                    url
                );

                if let Err(err) =
                    send_discord_notification(config.config.webhook_url.as_ref().unwrap(), &message)
                        .await
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_google_is_up() {
        let result = is_url_up("https://www.google.com", 5).await;
        assert!(result, "Expected Google to be up");
    }

    #[tokio::test]
    async fn test_nonexistent_url_is_down() {
        let result = is_url_up("http://nonexistent.subdomain.rust-lang.org", 5).await;
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

    #[test]
    fn test_load_config_from_toml() {
        // Define a valid TOML configuration string
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            discord_id = "1234567890"
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

            [sites]
            urls = [
                "https://www.google.com",
                "https://www.rust-lang.org",
                "https://invalid.url"
            ]
        "#;

        // Write the TOML content to a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "{}", toml_content).expect("Failed to write to temp file");

        // Parse the config
        let config = Config::load(temp_file.path()).expect("Failed to parse config");

        // Assertions
        assert_eq!(config.config.timeout_secs, 5);
        assert_eq!(config.config.check_interval_secs, 60);
        assert_eq!(config.sites.urls.len(), 3);
        assert_eq!(config.sites.urls[0], "https://www.google.com");
        assert_eq!(config.sites.urls[1], "https://www.rust-lang.org");
        assert_eq!(config.sites.urls[2], "https://invalid.url");
        assert_eq!(config.config.discord_id, Some("1234567890".to_string()));
        assert_eq!(
            config.config.webhook_url,
            Some("https://discord.com/api/webhooks/1234567890/abcdefg".to_string())
        );
    }
}
