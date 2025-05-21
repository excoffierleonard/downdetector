# Downdetector

A simple service that monitors website availability and sends Discord notifications when sites go down.

## Deployment

```bash
curl -o compose.yaml https://raw.githubusercontent.com/excoffierleonard/downdetector/refs/heads/main/compose.yaml
docker compose pull
docker compose up -d
```

## Configuration

Modify the default config.toml located in the docker volume of the application:

```toml
[config]
timeout_secs = 5
check_interval_secs = 60
discord_id = 1234567890
webhook_url = "https://discord.com/api/webhooks/1234567890/abcdefg"

[sites]
urls = [
    "https://www.google.com",
    "https://www.rust-lang.org",
    "https://invalid.url",
]
```

You may also override or directly define the private values by environment variable / .env:

- `DISCORD_ID`: The discord id of the user that will be tagged on the event of a notification
- `WEBHOOK_URL`: The api endpoint where to send the notification to. [More Information](https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks)

## Features

- Automated website availability monitoring
- Discord notifications for downtime alerts
- Configurable monitoring parameters
- Lightweight and efficient Rust implementation
- Docker-ready for simple deployment

## License

This project is licensed under the MIT License - see the LICENSE file for details.
