use crate::error::Error;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use url::Url;

const DEFAULT_CONFIG: &str = include_str!("../config.default.toml");

// Default values as constants
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CHECK_INTERVAL_SECS: u64 = 300;

/// Configuration structure for the downtime detector application.
///
/// This struct contains both the application configuration options
/// and the list of sites to monitor.
#[derive(Debug)]
pub struct Config {
    /// Application configuration options
    pub config: ConfigOptions,
    /// List of sites to monitor
    pub sites: SiteList,
}

/// Application configuration options.
///
/// These options control the behavior of the downtime detector,
/// including timeouts, check intervals, and Discord notification settings.
#[derive(Debug)]
pub struct ConfigOptions {
    /// HTTP request timeout in seconds.
    /// Must be greater than 0.
    pub timeout_secs: u64,
    /// Interval between site checks in seconds.
    /// Must be between 1 and 86399 (inclusive).
    pub check_interval_secs: u64,
    /// Discord webhook URL for sending notifications.
    /// Must be a valid Discord webhook URL starting with `https://discord.com/api/webhooks/`.
    /// Can also be set via the `WEBHOOK_URL` environment variable.
    pub webhook_url: Option<String>,
    /// Discord user ID for mentions in notifications.
    /// Can also be set via the `DISCORD_ID` environment variable.
    pub discord_id: Option<u64>,
}

/// List of sites to monitor.
///
/// Contains a vector of URLs that will be checked periodically
/// for availability.
#[derive(Debug, Deserialize, Default)]
pub struct SiteList {
    /// URLs to monitor for downtime.
    /// Each URL must be valid and parseable.
    #[serde(default)]
    pub urls: Vec<String>,
}

impl Config {
    /// Loads the configuration from the default config file location.
    ///
    /// The config file is expected to be at:
    /// - Linux/macOS: `~/.config/downdetector/config.toml`
    /// - Windows: `%APPDATA%\downdetector\config.toml`
    ///
    /// If the config file doesn't exist, a default one will be created.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config directory cannot be determined
    /// - The config file cannot be read or created
    /// - The TOML content is invalid
    /// - Any validation fails (invalid URLs, out-of-range values, etc.)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = Config::load().expect("Failed to load configuration");
    /// println!("Monitoring {} sites", config.sites.urls.len());
    /// ```
    pub fn load() -> Result<Self, Error> {
        let path = find_config()?;
        let content = fs::read_to_string(path)?;
        let raw: RawConfig = toml::from_str(&content)?;
        raw.try_into()
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    config: RawConfigOptions,
    #[serde(default)]
    sites: SiteList,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct RawConfigOptions {
    timeout_secs: u64,
    check_interval_secs: u64,
    webhook_url: Option<String>,
    discord_id: Option<u64>,
}

// Implement Default for RawConfigOptions
impl Default for RawConfigOptions {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            check_interval_secs: DEFAULT_CHECK_INTERVAL_SECS,
            webhook_url: None,
            discord_id: None,
        }
    }
}

// Builder pattern for validation
impl Config {
    fn validate_timeout(timeout_secs: u64) -> Result<u64, Error> {
        if timeout_secs == 0 {
            return Err(Error::Config("timeout_secs must be > 0".into()));
        }
        Ok(timeout_secs)
    }

    fn validate_check_interval(check_interval_secs: u64) -> Result<u64, Error> {
        if !(1..86400).contains(&check_interval_secs) {
            return Err(Error::Config(
                "check_interval_secs must be > 0 and < 86400".into(),
            ));
        }
        Ok(check_interval_secs)
    }

    fn validate_webhook_url(raw_url: Option<String>) -> Result<Option<String>, Error> {
        let webhook_url = match dotenvy::var("WEBHOOK_URL").ok().or(raw_url) {
            Some(url) if !url.trim().is_empty() => url,
            _ => return Ok(None),
        };

        let parsed_url = Url::parse(&webhook_url)
            .map_err(|_| Error::Config("Invalid webhook URL format".into()))?;

        if parsed_url.scheme() != "https"
            || parsed_url.host_str() != Some("discord.com")
            || !parsed_url.path().starts_with("/api/webhooks/")
        {
            return Err(Error::Config(
            "Webhook URL must be a valid Discord webhook starting with https://discord.com/api/webhooks/".into()
        ));
        }

        // Extract webhook ID from the path
        let path_parts: Vec<&str> = parsed_url
            .path()
            .strip_prefix("/api/webhooks/")
            .unwrap_or("")
            .split('/')
            .collect();

        // Ensure we have at least webhook_id/webhook_token
        if path_parts.len() < 2 || path_parts[0].is_empty() || path_parts[1].is_empty() {
            return Err(Error::Config(
                "Webhook URL must contain both webhook ID and token".into(),
            ));
        }

        // Validate webhook ID is a valid snowflake (parsable as u64)
        let webhook_id = path_parts[0];
        webhook_id.parse::<u64>().map_err(|_| {
            Error::Config(format!(
                "Invalid webhook ID '{webhook_id}': must be a valid Discord snowflake (numeric ID)"
            ))
        })?;

        Ok(Some(webhook_url))
    }

    fn validate_discord_id(raw_id: Option<u64>) -> Option<u64> {
        dotenvy::var("DISCORD_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(raw_id)
    }

    fn validate_urls(urls: &[String]) -> Result<(), Error> {
        for url in urls {
            Url::parse(url).map_err(|_| Error::Config(format!("Invalid URL: {url}")))?;
        }
        Ok(())
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Error> {
        // Validate all fields
        let timeout_secs = Config::validate_timeout(raw.config.timeout_secs)?;
        let check_interval_secs = Config::validate_check_interval(raw.config.check_interval_secs)?;
        let webhook_url = Config::validate_webhook_url(raw.config.webhook_url)?;
        let discord_id = Config::validate_discord_id(raw.config.discord_id);
        Config::validate_urls(&raw.sites.urls)?;

        Ok(Config {
            config: ConfigOptions {
                timeout_secs,
                check_interval_secs,
                webhook_url,
                discord_id,
            },
            sites: raw.sites,
        })
    }
}

fn find_config() -> Result<PathBuf, Error> {
    let config_path = dirs::config_dir()
        .ok_or_else(|| Error::Config("Unable to find config directory".into()))?
        .join("downdetector")
        .join("config.toml");

    if config_path.exists() {
        return Ok(config_path);
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&config_path, DEFAULT_CONFIG)?;
    Ok(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_CONFIG: &str = include_str!("../config.example.toml");

    #[test]
    fn default_config_is_valid() {
        let _config: Config = toml::from_str::<RawConfig>(DEFAULT_CONFIG)
            .expect("Failed to parse config")
            .try_into()
            .expect("Failed to convert to Config");
    }

    #[test]
    fn example_config_is_valid() {
        let _config: Config = toml::from_str::<RawConfig>(EXAMPLE_CONFIG)
            .expect("Failed to parse config")
            .try_into()
            .expect("Failed to convert to Config");
    }

    #[test]
    fn test_load_config_from_toml() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
            urls = [
                "https://www.google.com",
                "https://www.rust-lang.org",
                "https://invalid.url"
            ]
        "#;

        let config: Config = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into()
            .expect("Failed to convert to Config");

        assert_eq!(config.config.timeout_secs, 5);
        assert_eq!(config.config.check_interval_secs, 60);
        assert_eq!(config.sites.urls.len(), 3);
        assert_eq!(config.sites.urls[0], "https://www.google.com");
        assert_eq!(config.sites.urls[1], "https://www.rust-lang.org");
        assert_eq!(config.sites.urls[2], "https://invalid.url");
        assert_eq!(config.config.discord_id, Some(1_234_567_890));
        assert_eq!(
            config.config.webhook_url,
            Some("https://discord.com/api/webhooks/1234567890/abcdefg".to_string())
        );
    }

    #[test]
    fn test_partial_config_uses_defaults() {
        // Minimal config with defaults
        let toml_content = r#"
            [config]
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
            urls = ["https://www.google.com"]
        "#;

        let config: Config = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into()
            .expect("Failed to convert to Config");

        // Should use default values
        assert_eq!(config.config.timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(
            config.config.check_interval_secs,
            DEFAULT_CHECK_INTERVAL_SECS
        );
    }

    #[test]
    fn test_empty_config_section_uses_defaults() {
        // Config with empty sections
        let toml_content = r#"
            [config]
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
        "#;

        let config: Config = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into()
            .expect("Failed to convert to Config");

        // Sites should have empty URLs vector
        assert_eq!(config.sites.urls.len(), 0);
    }

    #[test]
    fn test_invalid_timeout() {
        let toml_content = r#"
            [config]
            timeout_secs = 0
            check_interval_secs = 60
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
            urls = ["https://www.google.com"]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid timeout");
    }

    #[test]
    fn test_invalid_check_interval() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 86400
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
            urls = ["https://www.google.com"]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid check interval");
    }

    #[test]
    fn test_invalid_webhook_url() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            webhook_url = "invalid-url"
            discord_id = 1234567890
            
            [sites]
            urls = ["https://www.google.com"]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid webhook URL");
    }

    #[test]
    fn test_invalid_monitored_url() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"
            discord_id = 1234567890
            
            [sites]
            urls = ["invalid-url"]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid URL");
    }
}
