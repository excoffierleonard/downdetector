//! A simple website downtime detector with Discord notifications.
//!
//! This crate provides functionality to monitor websites for availability
//! and send Discord webhook notifications when sites go down.
//!
//! # Features
//!
//! - Periodic monitoring of multiple websites
//! - Configurable check intervals and timeouts
//! - Discord webhook integration for downtime alerts
//! - Optional user mentions in Discord notifications
//! - Automatic configuration file creation with sensible defaults
//!
//! # Usage
//!
//! The main entry point is the [`monitor_websites`] function, which runs
//! continuously and monitors all configured websites.
//!
//! ```no_run
//! #[tokio::main]
//! async fn main() {
//!     // Initialize logging
//!     env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
//!     
//!     // Start monitoring (runs forever)
//!     downdetector::monitor_websites().await;
//! }
//! ```
//!
//! # Configuration
//!
//! The application expects a TOML configuration file at:
//! - Linux/macOS: `~/.config/downdetector/config.toml`
//! - Windows: `%APPDATA%\downdetector\config.toml`
//!
//! A default configuration file will be created if it doesn't exist.
//!
//! Example configuration:
//!
//! ```toml
//! [config]
//! timeout_secs = 30
//! check_interval_secs = 300
//! webhook_url = "https://discord.com/api/webhooks/..."
//! discord_id = 123456789
//!
//! [sites]
//! urls = [
//!     "https://example.com",
//!     "https://another-site.com"
//! ]
//! ```
//!
//! # Environment Variables
//!
//! The following environment variables can override config file values:
//! - `WEBHOOK_URL`: Discord webhook URL for notifications
//! - `DISCORD_ID`: Discord user ID for mentions

mod config;
mod error;
mod worker;

/// The main monitoring function that continuously checks website availability.
///
/// See the [module documentation](crate) for usage examples.
pub use worker::monitor_websites;
