use serde::Deserialize;
use std::{fs, path::Path};

use crate::errors::Error;

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
    pub fn load(path: &Path) -> Result<Config, Error> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;

        // if discord_id is not set uuse env with dotenvy
        if config.config.discord_id.is_none() {
            config.config.discord_id = Some(dotenvy::var("DISCORD_ID")?);
        }

        // if webhook_url is not set use env with dotenvy
        if config.config.webhook_url.is_none() {
            config.config.webhook_url = Some(dotenvy::var("WEBHOOK_URL")?);
        }

        Ok(config)
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
        assert_eq!(config.config.discord_id, Some("1234567890".to_string()));
        assert_eq!(
            config.config.webhook_url,
            Some("https://discord.com/api/webhooks/1234567890/abcdefg".to_string())
        );
    }
}
