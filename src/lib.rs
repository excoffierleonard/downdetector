use reqwest::Client;
use std::time::Duration;
use tokio::time;

pub mod config;

/// Asynchronously checks if a given URL is up (returns a 2xx status).
pub async fn is_url_up(url: &str, timeout_secs: u64) -> bool {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap();

    client
        .get(url)
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}

/// Monitors websites periodically and prints their status
pub async fn monitor_websites(urls: Vec<String>, timeout_secs: u64, check_interval_secs: u64) {
    loop {
        println!("Checking website status...");

        for url in &urls {
            let status = is_url_up(url, timeout_secs).await;
            let status_text = if status { "UP" } else { "DOWN" };
            println!("{}: {}", url, status_text);
        }

        // Sleep for the configured interval before the next check
        time::sleep(Duration::from_secs(check_interval_secs)).await;
    }
}

pub async fn send_discord_notification(
    webhook_url: &str,
    message: &str,
) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let payload = serde_json::json!({
        "content": message,
    });

    client.post(webhook_url).json(&payload).send().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_google_is_up() {
        let result = is_url_up("https://www.google.com", 5).await;
        assert!(result, "Expected Google to be up");
    }

    #[tokio::test]
    async fn test_nonexistent_url_is_down() {
        let result = is_url_up("http://nonexistent.subdomain.rust-lang.org", 5).await;
        assert!(!result, "Expected nonexistent URL to be down");
    }

    #[tokio::test]
    async fn test_discord_notification() {
        let webhook_url = "https://discord.com/api/webhooks/1277781729754484777/AUQiUl_M_suiJ8u7QIDOab1gft7_KFnTXwX93UQZmqPWKhqgNu7a7o0rLe_Zb71EgC-R";
        let discord_id = dotenvy::var("DISCORD_ID").expect("DISCORD_ID not set");
        let message = format!("{} Test notification from Rust!", discord_id);
        let result = send_discord_notification(webhook_url, &message).await;
        assert!(
            result.is_ok(),
            "Expected notification to be sent successfully"
        );
    }
}
