use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use std::{fs, path::Path, time::Duration};
use tokio::time;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub config: ConfigOptions,
    pub sites: SiteList,
}

#[derive(Debug, Deserialize)]
pub struct ConfigOptions {
    pub timeout_secs: u64,
    pub check_interval_secs: u64,
    pub discord_id: Option<String>,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SiteList {
    pub urls: Vec<String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        // if discord_id is not set uuse env with dotenvy
        if config.config.discord_id.is_none() {
            let discord_id =
                dotenvy::var("DISCORD_ID").expect("DISCORD_ID environment variable not set");
            config.config.discord_id = Some(discord_id);
        }

        // if webhook_url is not set use env with dotenvy
        if config.config.webhook_url.is_none() {
            let webhook_url =
                dotenvy::var("WEBHOOK_URL").expect("WEBHOOK_URL environment variable not set");
            config.config.webhook_url = Some(webhook_url);
        }

        Ok(config)
    }
}

/// Asynchronously checks if a given URL is up (returns a 2xx status).
pub async fn is_url_up(url: &str, timeout_secs: u64) -> bool {
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

/// Monitors websites periodically and prints their status
pub async fn monitor_websites(
    urls: Vec<String>,
    timeout_secs: u64,
    check_interval_secs: u64,
    discord_id: String,
    webhook_url: String,
) {
    loop {
        println!("Checking website status...");

        for url in &urls {
            let status = is_url_up(url, timeout_secs).await;
            let status_text = if status { "UP" } else { "DOWN" };
            println!("{}: {}", url, status_text);

            if !status {
                let message = format!("<@{}> Alert: {} is DOWN!", discord_id, url);

                if let Err(err) = send_discord_notification(&webhook_url, &message).await {
                    eprintln!("Failed to send Discord notification: {}", err);
                }
            }
        }

        // Sleep for the configured interval before the next check
        time::sleep(Duration::from_secs(check_interval_secs)).await;
    }
}

#[derive(Serialize)]
struct DiscordMessage {
    content: String,
}

pub async fn send_discord_notification(
    webhook_url: &str,
    message: &str,
) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let payload = DiscordMessage {
        content: message.to_string(),
    };

    client.post(webhook_url).json(&payload).send().await?;
    Ok(())
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
