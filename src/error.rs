use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Environment variable fetching error: {0}")]
    EnvVarNotSet(#[from] dotenvy::Error),
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),
}
