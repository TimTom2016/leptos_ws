# Leptos Websocket

[![Crates.io](https://img.shields.io/crates/v/leptos_ws.svg)](https://crates.io/crates/leptos_ws)
[![docs.rs](https://docs.rs/leptos_ws/badge.svg)](https://docs.rs/leptos_ws/)

Leptos Websocket provides real-time, reactive signals for [Leptos](https://github.com/leptos-rs/leptos), keeping them in sync with the server through WebSockets. This enables instant UI updates controlled by the server or shared between client and server.

## Features

- **Server Signals**: Read-only on the client, writable by the server.
- **Bidirectional Signals**: Read-write on both client and server; updates are synchronized in both directions.
- **Channel Signals**: Message-based channels for sending and receiving discrete messages between client and server.
- **Real-time Updates**: Changes to signals are sent through WebSockets as [JSON patches](https://docs.rs/json-patch/latest/json_patch/struct.Patch.html).
- **Framework Integration**: Supports integration with [Axum](https://github.com/tokio-rs/axum) and [Actix Web](https://github.com/actix/actix-web).

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
leptos_ws = "0.9.0"
serde = { version = "1.0", features = ["derive"] }

[features]
hydrate = ["leptos_ws/hydrate"]
ssr = ["leptos_ws/ssr"]
```

## Usage

```rust
use leptos::prelude::*;
#[cfg(any(feature = "ssr", feature = "hydrate"))]
use leptos_ws::ReadOnlySignal;
use serde::{Deserialize, Serialize};

#[component]
#[cfg(feature = "hydrate")]
pub fn App() -> impl IntoView {
    // Connect to WebSocket
    
    leptos_ws::provide_websocket();

    // Create a read-only server signal (updated by the server)
    let count = ReadOnlySignal::new("count", 0 as i32).unwrap();

    view! {
        <button on:click=move |_| {
            // Call the server function to start updating the count
            leptos::task::spawn_local(async move {
                update_count().await.unwrap();
            });
        }>"Start Counter"</button>
        <h1>"Count: " {move || count.get().to_string()}</h1>
    }
}

#[server]
async fn update_count() -> Result<(), leptos::prelude::ServerFnError> {
#[cfg(feature = "ssr")]
    use std::time::Duration;
    use tokio::time::sleep;
    let count = ReadOnlySignal::new("count", 0 as i32).unwrap();
    for i in 0..100 {
        count.update(|value| *value = i);
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}
```

## Feature Flags

- `ssr`: Enable server-side rendering support.
- `hydrate`: Enable hydration support.
- `csr`: Enable client-side rendering support.

## Documentation

For more detailed information, check out the [API documentation](https://docs.rs/leptos_ws/).

## Compatible Leptos versions

The `main` branch is compatible with the latest Leptos release.

Compatibility of `leptos_ws` versions:

| `leptos_ws` | `leptos` |
| :--         | :--      |
| `0.8`-`0.9` | `0.8`    |
| `0.7`       | `0.7`    |

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
