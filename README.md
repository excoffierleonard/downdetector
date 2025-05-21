# Downdetector

A simple service that monitors website availability and sends Discord notifications when sites go down.

## Overview

Downdetector continuously checks if your configured websites are responding with a successful HTTP status code (2xx). If a site becomes unavailable, it immediately sends a notification to a specified Discord user via webhook.

## Features

- Automated website availability monitoring
- Discord notifications for downtime alerts
- Configurable monitoring parameters
- Lightweight and efficient Rust implementation
- Docker-ready for simple deployment

## Architecture

The service is built in Rust using:

- Tokio for asynchronous operations
- Reqwest for HTTP requests
- Serde for configuration parsing
- Discord webhooks for notifications

## Configuration

Configuration is managed through a TOML file (located at the default configuration space of your OS) that defines:

- Websites to monitor
- Check frequency and request timeouts
- Discord webhook URL and user ID to notify

## Deployment

```bash
docker compose pull ghcr.io/excoffierleonard/downdetector:latest
docker compose up -d
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.
