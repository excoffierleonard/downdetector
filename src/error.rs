use thiserror::Error;

/// The main error type for the downtime detector application.
///
/// This enum represents all possible errors that can occur during
/// the application's execution, from configuration loading to HTTP
/// requests and Discord notifications.
#[derive(Error, Debug)]
pub enum Error {
    /// An I/O error occurred.
    ///
    /// This typically happens when reading/writing configuration files
    /// or creating directories.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse TOML configuration.
    ///
    /// Occurs when the configuration file contains invalid TOML syntax
    /// or structure.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Failed to read an environment variable.
    ///
    /// Occurs when attempting to load environment variables for
    /// webhook URLs or Discord IDs.
    #[error("Environment variable fetching error: {0}")]
    EnvVarNotSet(#[from] dotenvy::Error),

    /// HTTP request failed.
    ///
    /// Occurs during site availability checks or when sending
    /// Discord webhook notifications.
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),

    /// Configuration validation failed.
    ///
    /// Occurs when configuration values don't meet requirements,
    /// such as invalid timeout values, check intervals out of range,
    /// or malformed webhook URLs.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Failed to parse a URL.
    ///
    /// Occurs when parsing monitored site URLs or webhook URLs
    /// that have invalid format.
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Failed to parse an integer.
    ///
    /// Typically occurs when parsing Discord IDs from environment
    /// variables.
    #[error("Integer parsing error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}
