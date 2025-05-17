use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub config: ConfigOptions,
    pub sites: SiteList,
}

#[derive(Debug, Deserialize)]
pub struct ConfigOptions {
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct SiteList {
    pub urls: Vec<String>,
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
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
        let config = load_config(temp_file.path()).expect("Failed to parse config");

        // Assertions
        assert_eq!(config.config.timeout_secs, 5);
        assert_eq!(config.sites.urls.len(), 3);
        assert_eq!(config.sites.urls[0], "https://www.google.com");
        assert_eq!(config.sites.urls[1], "https://www.rust-lang.org");
        assert_eq!(config.sites.urls[2], "https://invalid.url");
    }
}
