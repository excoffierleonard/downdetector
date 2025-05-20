use serde::Deserialize;
use std::convert::TryFrom;
use std::{fs, path::Path};
use url::Url;

use crate::errors::Error;

/// Finalized runtime config — no more `Option`
#[derive(Debug)]
pub struct Config {
    pub config: ConfigOptions,
    pub sites: SiteList,
}

#[derive(Debug)]
pub struct ConfigOptions {
    pub timeout_secs: u64,
    pub check_interval_secs: u64,
    pub discord_id: String,
    pub webhook_url: String,
}

#[derive(Debug, Deserialize)]
pub struct SiteList {
    pub urls: Vec<String>,
}

/// Raw version for TOML deserialization — can contain missing fields
#[derive(Debug, Deserialize)]
struct RawConfig {
    config: RawConfigOptions,
    sites: SiteList,
}

#[derive(Debug, Deserialize)]
struct RawConfigOptions {
    timeout_secs: u64,
    check_interval_secs: u64,
    discord_id: Option<String>,
    webhook_url: Option<String>,
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        // Discord ID validation
        let discord_id = raw
            .config
            .discord_id
            .or_else(|| dotenvy::var("DISCORD_ID").ok())
            .ok_or_else(|| Error::Config("Missing discord_id in file or env".into()))?;

        if !discord_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::Config("discord_id must be a valid snowflake".into()));
        }

        // Webhook URL validation
        let webhook_url = raw
            .config
            .webhook_url
            .or_else(|| dotenvy::var("WEBHOOK_URL").ok())
            .ok_or_else(|| Error::Config("Missing webhook_url in file or env".into()))?;

        validate_webhook_url(&webhook_url)?;

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

fn validate_webhook_url(url_str: &str) -> Result<(), Error> {
    let url = Url::parse(url_str)?;

    if url.scheme() != "https" {
        return Err(Error::Config("webhook_url must use https scheme".into()));
    }

    if !url_str.starts_with("https://discord.com/api/webhooks/") {
        return Err(Error::Config(
            "webhook_url must start with https://discord.com/api/webhooks/".into(),
        ));
    }

    Ok(())
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let content = fs::read_to_string(path)?;
        let raw: RawConfig = toml::from_str(&content)?;
        raw.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
        assert_eq!(config.config.discord_id, "1234567890".to_string());
        assert_eq!(
            config.config.webhook_url,
            "https://discord.com/api/webhooks/1234567890/abcdefg".to_string()
        );
    }
}
