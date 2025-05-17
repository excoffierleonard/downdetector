use reqwest::Client;
use std::time::Duration;

pub mod config;

/// Asynchronously checks if a given URL is up (returns a 2xx status).
pub async fn is_url_up(url: &str) -> bool {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    client
        .get(url)
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_google_is_up() {
        let result = is_url_up("https://www.google.com").await;
        assert!(result, "Expected Google to be up");
    }

    #[tokio::test]
    async fn test_nonexistent_url_is_down() {
        let result = is_url_up("http://nonexistent.subdomain.rust-lang.org").await;
        assert!(!result, "Expected nonexistent URL to be down");
    }
}
