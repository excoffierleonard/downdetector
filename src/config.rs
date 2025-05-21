use serde::Deserialize;
use std::convert::TryFrom;
use std::{fs, path::PathBuf};
use url::Url;

use crate::error::Error;

const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[derive(Debug)]
pub struct Config {
    pub config: ConfigOptions,
    pub sites: SiteList,
}

#[derive(Debug)]
pub struct ConfigOptions {
    pub timeout_secs: u64,
    pub check_interval_secs: u64,
    pub discord_id: u64,
    pub webhook_url: String,
}

#[derive(Debug, Deserialize)]
pub struct SiteList {
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
    config: RawConfigOptions,
    sites: SiteList,
}

#[derive(Debug, Deserialize)]
struct RawConfigOptions {
    timeout_secs: u64,
    check_interval_secs: u64,
    discord_id: Option<u64>,
    webhook_url: Option<String>,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        // Discord ID validation: Environment variable overrides file configuration.
        let discord_id: u64 = dotenvy::var("DISCORD_ID")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(raw.config.discord_id)
            .ok_or_else(|| Error::Config("Missing discord_id in env or file".into()))?;

        // Webhook URL validation: Environment variable overrides file configuration.
        let webhook_url = dotenvy::var("WEBHOOK_URL")
            .ok()
            .or(raw.config.webhook_url)
            .ok_or_else(|| Error::Config("Missing webhook_url in env or file".into()))?;

        let parsed_url = Url::parse(&webhook_url)
            .map_err(|_| Error::Config("Invalid webhook URL format".into()))?;

        if parsed_url.scheme() != "https"
            || parsed_url.host_str() != Some("discord.com")
            || !parsed_url.path().starts_with("/api/webhooks/")
        {
            return Err(Error::Config("Webhook URL must be a valid Discord webhook starting with https://discord.com/api/webhooks/".into()));
        }

        // Timout validation
        if raw.config.timeout_secs == 0 {
            return Err(Error::Config("timeout_secs must be > 0".into()));
        }

        // Check interval validation
        if !(1..86400).contains(&raw.config.check_interval_secs) {
            return Err(Error::Config(
                "check_interval_secs must be > 0 and < 86400".into(),
            ));
        }

        // Validate monitored sites URLs
        for url in &raw.sites.urls {
            if Url::parse(url).is_err() {
                return Err(Error::Config(format!("Invalid URL: {}", url)));
            }
        }

        Ok(Config {
            config: ConfigOptions {
                timeout_secs: raw.config.timeout_secs,
                check_interval_secs: raw.config.check_interval_secs,
                discord_id,
                webhook_url,
            },
            sites: raw.sites,
        })
    }
}

fn find_config() -> Result<PathBuf, Error> {
    let config_path = dirs::config_dir()
        // TODO: Better error handling
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

    #[test]
    fn test_load_config_from_toml() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            discord_id = 1234567890
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

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
        assert_eq!(config.config.discord_id, 1234567890);
        assert_eq!(
            config.config.webhook_url,
            "https://discord.com/api/webhooks/1234567890/abcdefg".to_string()
        );
    }

    // Test invalide timeout
    #[test]
    fn test_invalid_timeout() {
        let toml_content = r#"
            [config]
            timeout_secs = 0
            check_interval_secs = 60
            discord_id = 1234567890
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

            [sites]
            urls = [
                "https://www.google.com"
            ]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid timeout");
    }

    // Test invalid check interval
    #[test]
    fn test_invalid_check_interval() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 86400
            discord_id = 1234567890
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

            [sites]
            urls = [
                "https://www.google.com"
            ]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid check interval");
    }

    // NOTE: We do not test for invalid Discord ID since it is enforced by the type system, a snowflake ID is a u64.

    // Test invalid webhook URL
    #[test]
    fn test_invalid_webhook_url() {
        let toml_content = r#"
            [config]
            timeout_secs = 5
            check_interval_secs = 60
            discord_id = 1234567890
            webhook_url = "invalid-url"

            [sites]
            urls = [
                "https://www.google.com"
            ]
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
            discord_id = 1234567890
            webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

            [sites]
            urls = [
                "invalid-url"
            ]
        "#;

        let result: Result<Config, Error> = toml::from_str::<RawConfig>(toml_content)
            .expect("Failed to parse config")
            .try_into();

        assert!(result.is_err(), "Expected error for invalid URL");
    }
}
