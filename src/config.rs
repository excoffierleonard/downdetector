use crate::error::Error;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use url::Url;

const DEFAULT_CONFIG: &str = include_str!("../config.default.toml");

// Default values as constants
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CHECK_INTERVAL_SECS: u64 = 300;

#[derive(Debug)]
pub struct Config {
    pub config: ConfigOptions,
    pub sites: SiteList,
}

#[derive(Debug)]
pub struct ConfigOptions {
    pub timeout_secs: u64,
    pub check_interval_secs: u64,
    pub webhook_url: Option<String>,
    pub discord_id: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SiteList {
    #[serde(default)]
    pub urls: Vec<String>,
}

impl Config {
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
            Some(url) => url,
            None => return Ok(None),
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

        Ok(Some(webhook_url))
    }

    fn validate_discord_id(raw_id: Option<u64>) -> Result<Option<u64>, Error> {
        Ok(dotenvy::var("DISCORD_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(raw_id))
    }

    fn validate_urls(urls: &[String]) -> Result<(), Error> {
        for url in urls {
            Url::parse(url).map_err(|_| Error::Config(format!("Invalid URL: {}", url)))?;
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
        let discord_id = Config::validate_discord_id(raw.config.discord_id)?;
        Config::validate_urls(&raw.sites.urls)?;

        Ok(Config {
            config: ConfigOptions {
                timeout_secs,
                check_interval_secs,
                discord_id,
                webhook_url,
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
        assert_eq!(config.config.discord_id, Some(1234567890));
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
