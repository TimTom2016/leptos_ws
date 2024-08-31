Here's an improved version of your README:

# Leptos Websocket

[![Crates.io](https://img.shields.io/crates/v/leptos_ws.svg)](https://crates.io/crates/leptos_ws)
[![docs.rs](https://docs.rs/leptos_ws/badge.svg)](https://docs.rs/leptos_ws/)

Leptos Websocket provides server signals for [Leptos](https://github.com/leptos-rs/leptos), keeping them in sync with the server through WebSockets. This enables real-time updates on the UI controlled by the server.

## Features

- **Server Signals**: Read-only signals on the client side, writable by the server.
- **Real-time Updates**: Changes to signals are sent through WebSockets as [JSON patches](https://docs.rs/json-patch/latest/json_patch/struct.Patch.html).
- **Framework Integration**: Supports integration with the [Axum](https://github.com/tokio-rs/axum) web framework.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
leptos_ws = "0.1.0"
serde = { version = "1.0", features = ["derive"] }

[features]
ssr = ["leptos_ws/ssr", "leptos_ws/axum"]
```

## Usage

### Client-side

```rust
use leptos::*;
use serde::{Deserialize, Serialize};

#[component]
pub fn App() -> impl IntoView {
    // Connect to WebSocket
    leptos_ws::provide_websocket("http://localhost:3000/ws");

    // Create server signal
    let count = leptos_ws::ServerSignal::new("count".to_string(), 0 as i32).unwrap();

    view! {
        <h1>"Count: " {move || count.get().to_string()}</h1>
    }
}
```

### Server-side (Axum)

Server-side implementation requires additional setup. Refer to the example for detailed examples.

## Feature Flags

- `ssr`: Enable server-side rendering support.
- `axum`: Enable integration with the Axum web framework.

## Documentation

For more detailed information, check out the [API documentation](https://docs.rs/leptos_ws/).

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
