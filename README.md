# 📨 Jabber Webhook

A lightweight HTTP-to-XMPP webhook service written in Rust. It receives JSON payloads via HTTP POST and forwards them as XMPP (Jabber) messages to a configured recipient.

Perfect for integrating XMPP notifications into CI/CD pipelines, monitoring systems, contact forms, or any application that needs to send instant messages via a simple HTTP API.

## ✨ Features

- 🚀 **Async & Fast** — Built on [Axum](https://github.com/tokio-rs/axum) and [Tokio](https://tokio.rs/)
- 💬 **XMPP/Jabber Support** — Uses [tokio-xmpp](https://gitlab.com/xmpp-rs/xmpp-rs) for reliable message delivery
- 🔐 **Secure Configuration** — Credentials stored in `.env` file (never hardcoded)
- 🔄 **Auto-reconnect** — The XMPP client automatically reconnects on connection loss
- 📊 **Structured Logging** — Powered by [tracing](https://github.com/tokio-rs/tracing)
- 🐳 **Easy to Deploy** — Single binary, minimal dependencies

## 📋 Requirements

- Rust 1.70+ (edition 2021)
- An XMPP account for the bot (e.g., on `jabber.vg`, `conversations.im`, `xmpp.rs`, or your own server)
- A recipient XMPP JID to forward messages to

## 🛠️ Installation

### From source

```bash
git clone https://github.com/marchcat73/jabber-webhook.git
cd jabber-webhook
cargo build --release
```
## Configuration

```sh
# Bot credentials
BOT_JID=your_bot@jabber.vg
BOT_PASSWORD='your_password_here'

# Recipient JID (where messages will be sent)
RECIPIENT_JID=recipient@jabber.vg

# HTTP server settings
SERVER_HOST=127.0.0.1
SERVER_PORT=8082
```
## Important: If your password contains special characters like $, #, or spaces, wrap it in single quotes to prevent shell expansion:
```sh
BOT_PASSWORD='MyP@ss$word#123'
```
# Usage
## Start the server
```sh
cargo run --release
```
## Test
```sh
curl -X POST http://127.0.0.1:8082/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "name": "John Doe",
    "email": "john@example.com",
    "message": "Hello from the webhook!"
  }'
```

## Running with systemd (optional)
Create /etc/systemd/system/jabber-webhook.service:
```sh
[Unit]
Description=Jabber Webhook Service
After=network.target

[Service]
Type=simple
User=your_user
Group=your_user
WorkingDirectory=/path/to/jabber-webhook
EnvironmentFile=/path/to/jabber-webhook/.env
ExecStart=/path/to/jabber-webhook/target/release/jabber-webhook
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```
Then:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now jabber-webhook
```
🏗️ Project Structure
```sh
jabber-webhook/
├── Cargo.toml          # Dependencies
├── .env                # Configuration (not in git)
├── .env.example        # Example configuration
├── src/
│   └── main.rs         # Application entry point
└── README.md
```
🔧 Troubleshooting
NotAuthorized error

    Check that your BOT_JID and BOT_PASSWORD are correct
    If your password contains $, wrap it in single quotes in .env
    Try logging in with the same credentials using a desktop client like Gajim
     or Conversations

resource found while parsing a bare JID

    BOT_JID must be a bare JID (e.g., user@domain.com), without a resource part (/bot)
    ❌ BOT_JID=user@domain.com/bot
    ✅ BOT_JID=user@domain.com



📄 License
This project is licensed under the MIT License — see the LICENSE
 file for details.
🤝 Contributing
Contributions, issues, and feature requests are welcome! Feel free to open an issue or submit a pull request.

🙏 Acknowledgments

tokio-rs — Async runtime
xmpp-rs — XMPP library
Axum - Web framework
