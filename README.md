# SMS Terminal

A Rust TUI that send and receive SMS messages live all through your own hardware!
Related projects:
- [sms-server](https://github.com/morgverd/sms-server) ([crates.io](https://crates.io/crates/sms-server)) - SMS server that runs on a Raspberry Pi to accept and send messages with database storage
- [sms-client](https://github.com/morgverd/sms-client) ([crates.io](https://crates.io/crates/sms-client)) - A Rust library for remotely interfacing with the SMS server

## Features
- [Highly configurable](#configuration) with command line arguments, or a config file, or both!
- [Easy installation](#installation) with cargo install.
- [Phonebook / Messages](#phonebook) with recent contacts, friendly name support (with editing) and lazy loading.
- [Message sending](#sending--receiving) with multipart support, with live message receiving.
- [Device info](#device-info) that includes all basic modem connection information.
- [Delivery report](#delivery-reports) viewing (see when the recipient reported delivery).
- [Theme support](#themes) across all views.

> [!TIP]
> The WebSocket connection is entirely optional, however is strongly recommended for live updates.
> Alternatively, the message view can be refreshed manually if the websocket is disabled on the server.

## Configuration

| Name              | Type                                            | Help Text                                                                           |
|-------------------|-------------------------------------------------|-------------------------------------------------------------------------------------|
| `theme`           | `emerald, blue, zinc, indigo, red, amber, pink` | Select a built-in theme to start with.                                              |
| `host`            | String                                          | Set the server host for HTTP and WebSocket (e.g localhost:3000)                     |
| `http-uri`        | URI                                             | Set the HTTP URI, this overrides the host if set (e.g. http://localhost:3000/)      |
| `ws-uri`          | URI                                             | Set the WebSocket URI, this overrides the host if set (e.g. ws://localhost:3000/ws) |
| `ws-enabled`      | bool                                            | Enable WebSocket support.                                                           |
| `auth`            | String                                          | Authorization token to use for HTTP and WebSocket requests.                         |
| `ssl-certificate` | Path                                            | An SSL certificate filepath to use for SMS connections.                             |

These options can be supplied as command line arguments when running (`sms-server --theme red`) or in a config file stored at one of these locations:
 - **Windows**: `%appdata%/Local/sms-terminal/config.toml`
 - **Linux**: `HOME/.config/sms-terminal/config.toml`

```shell
# Example connecting to an insecure sms-server with authorization.
sms-terminal --host 192.168.1.20:3000 --auth hello --ws-enabled

# Example connecting to a secure sms-server without authorization.
sms-terminal --host sms-api.internal:3000 --ssl-certificate ./ca.crt

# Example viewing the messages of "+44123" insecurely.
sms-terminal messages "+44123" --host 192.168.1.20:3000
```

## Installation

```shell
cargo install sms-terminal
sms-terminal --host 192.168.1.20:3000
```

```shell
ubuntu@my-computer:/mnt/c/Users/Dell/Videos$ sms-terminal -h
A terminal-based SMS client that can send and receive messages live.

Usage: sms-terminal [OPTIONS] [COMMAND]

Commands:
  messages   Start on Messages view with a target state
  compose    Start on Compose SMS view with a target state
  phonebook  Start on Phonebook view
  help       Print this message or the help of the given subcommand(s)

Options:
      --theme <THEME>
          Select a built-in theme to start with [possible values: emerald, blue, zinc, indigo, red, amber, pink]
      --host <HOST>
          Set the server host for HTTP and WebSocket (e.g localhost:3000)
      --http-uri <HTTP_URI>
          Set the HTTP URI, this overrides the host if set (e.g. http://localhost:3000)
      --ws-uri <WS_URI>
          Set the WebSocket URI, this overrides the host if set (e.g. ws://localhost:3000/ws)
      --ws-enabled
          Enable WebSocket support
      --auth <AUTH>
          Authorization token to use for HTTP and WebSocket requests
      --ssl-certificate <SSL_CERTIFICATE>
          An SSL certificate filepath to use for SMS connections
  -h, --help
          Print help
  -V, --version
          Print version
```

![Installation](/.github/assets/installation.gif)

## Phonebook

The phonebook displays all recent contacts, and allows for the friendly name to be edited.
This is synced across all clients using the sms-server and will work across all UI that implements it (not just the terminal!).
The messages view also supports lazy loading via pagination, meaning only the messages being viewed are loaded.

![Phonebook](/.github/assets/phonebook-messages.gif)

## Sending / Receiving

Messages can be sent directly from the terminal, with newly received messages updating the table live (if viewing) or displaying a
notification that will show on any view and allow for easy switching to the new message.

![Sending](/.github/assets/sending.gif)
![Incoming Message](/.github/assets/incoming-message-notification.gif)

## Device Info

View all modem device information, including server version and signal strength etc.
The WebSocket connection is also monitored, with notifications sent to reflect its connection state.

![Device Info](/.github/assets/device-info.png)
![Websocket Reconnection](/.github/assets/websocket-reconnect.gif)

### Delivery Reports

Message delivery reports can be checked directly from the messages view. These only apply to outgoing messages.

![Delivery Reports](/.github/assets/delivery-reports.png)

### Themes

Full theme support across all views, with optional background filling (defaults on).
- **F10** - Change color
- **F11** - Change background fill mode

![Themes](/.github/assets/themes.gif)
