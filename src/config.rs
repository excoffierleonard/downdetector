use serde::Deserialize;
use std::{fs, path::Path};

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
