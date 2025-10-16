# SMS Terminal

A Rust-based TUI for sending and receiving SMS messages live through your own hardware.

## Related Projects

- **[sms-server](https://github.com/morgverd/sms-server)** ([crates.io](https://crates.io/crates/sms-server)) - SMS server for Raspberry Pi with database storage.
- **[sms-client](https://github.com/morgverd/sms-client)** ([crates.io](https://crates.io/crates/sms-client)) - Rust library for remote server interfacing.

## Features

- **[Smart Phonebook](#phonebook--messages)** - Recent contacts, friendly names with editing, and lazy loading.
- **[Live Messaging](#live-messaging)** - Send multipart messages and receive updates in real-time.
- **[Device Monitoring](#device-information)** - View modem connection info and signal strength.
- **[Delivery Reports](#delivery-reports)** - Track when recipients receive your messages.
- **[Theme Support](#themes)** - Multiple built-in themes with customization options.
- **[Easy Installation](#installation)** - Simple setup with `cargo install`.
- **[Highly Configurable](#configuration)** - Via command line arguments, config file, or both.
- **Error Reporting** - Optional Sentry integration for debugging.

## Quick Start

```bash
# Install via cargo
cargo install sms-terminal

# Connect to an insecure server with auth
sms-terminal --host 192.168.1.20:3000 --auth hello --ws-enabled

# Connect to a secure server
sms-terminal --host sms-api.internal:3000 --ssl-certificate ./ca.crt
```

## Showcase

### Phonebook & Messages
Access recent contacts with friendly name support. Messages are lazy-loaded for optimal performance.

![Phonebook](/.github/assets/phonebook-messages.gif)

### Live Messaging
Send messages and receive notifications in real-time across any view when WebSocket is enabled.

![Sending](/.github/assets/sending.gif)
![Incoming Message](/.github/assets/incoming-message-notification.gif)

### Device Information
Monitor modem status, server version, signal strength, and WebSocket connection state.

![Device Info](/.github/assets/device-info.png)
![Websocket Reconnection](/.github/assets/websocket-reconnect.gif)

### Delivery Reports
Check delivery confirmations for outgoing messages directly from the messages view.

![Delivery Reports](/.github/assets/delivery-reports.png)

### Themes
Customize your experience with built-in themes and background fill options.
- **F10** - Change color scheme
- **F11** - Toggle background fill mode

![Themes](/.github/assets/themes.gif)

## Configuration

Configuration can be provided through command line arguments or a config file.

### Config File Locations
- **Windows**: `%appdata%/Local/sms-terminal/config.toml`
- **Linux**: `$HOME/.config/sms-terminal/config.toml`

### Available Options

| Option            | Type                                                        | Description                                                 |
|-------------------|-------------------------------------------------------------|-------------------------------------------------------------|
| `theme`           | `emerald`, `blue`, `zinc`, `indigo`, `red`, `amber`, `pink` | Select a built-in theme                                     |
| `host`            | String                                                      | Server host for HTTP and WebSocket (e.g., `localhost:3000`) |
| `http-uri`        | URI                                                         | HTTP URI (overrides host if set)                            |
| `ws-uri`          | URI                                                         | WebSocket URI (overrides host if set)                       |
| `ws-enabled`      | Boolean                                                     | Enable WebSocket support for live updates                   |
| `auth`            | String                                                      | Authorization token for requests                            |
| `ssl-certificate` | Path                                                        | SSL certificate filepath for secure connections             |
| `sentry`          | URI                                                         | Sentry DSN for error reporting (requires `sentry` feature)  |

> [!TIP]
> WebSocket connection is optional but strongly recommended for live updates!

## Installation

### Basic Installation
```bash
cargo install sms-terminal
```

### With Sentry Support
```bash
cargo install sms-terminal -F sentry
```

### Usage Examples
```bash
# View help and available commands
sms-terminal -h

# Start with messages view for a specific contact
sms-terminal messages "+44123" --host 192.168.1.20:3000

# Start in compose mode
sms-terminal compose --host 192.168.1.20:3000

# Start with phonebook view
sms-terminal phonebook --host 192.168.1.20:3000
```

## Commands

- `messages` - Start with Messages view for a specific contact.
- `compose` - Start with Compose SMS view.
- `phonebook` - Start with Phonebook view.
- `help` - Display help information.
