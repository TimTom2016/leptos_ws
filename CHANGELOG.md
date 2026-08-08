# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- **Per-connection state**: `ChannelSignal<T, S>` now carries a per-connection state `S`, created via a factory (`with_state_factory`) and passed to handlers through `ChannelContext`, which also exposes the `client_id`.
- **Send mapper**: `add_send_mapper` lets you transform or filter outgoing channel messages per-connection (`None` suppresses delivery for that client).
- **Send filter**: `add_send_filter` lets you suppress outgoing channel messages per-connection. Serializes once instead of per connection, so prefer it over a mapper for pure filtering.
- **`axum-adv` example**: Full example demonstrating per-connection state, send mappers, and server/client handlers.
- `ChannelHandler` now accepts closures with any state type, not just `()`.

### Fixed
- Panic when reading a disposed signal in `update_if_changed` — now uses `try_get` and returns an error instead.
- Deadlock when a channel message handler calls `send_message` (per-connection entries are now removed before handling and re-inserted afterwards).
- Dropped channel messages are now logged instead of silently ignored.
- Various import and documentation cleanups.

## [0.9.8] - 2026-06-16
### Added
- Server-side tracing events for WebSocket connections, messages, and broadcasts (feature-gated behind `ssr`).
- Client-side `delete()` now sends an unsubscribe message to the server, which stops broadcasting to that client without affecting other clients.

### Fixed
- Server no longer panics when `WsSignals` context is missing — returns a proper error instead.
- `get_signal`/`get_channel` no longer panic on type mismatch — returns `None` instead.
- `MutexGuard` no longer held across `.await` in the reconnection loop (potential deadlock fix).
- `RwLockReadGuard` no longer held across `.await` in `ClientBidirectionalSignal::update_if_changed`.
- `tx.send()` failures are now handled gracefully instead of panicking.
- Server `Delete` handler now aborts the per-client broadcast task instead of deleting the signal globally.
- `on_server`/`on_client` no-op methods simplified to return `()` instead of `Result<(), Error>`.
- Unused `Result` from `delete_channel` now explicitly discarded.

## [0.9.7] - 2026-02-02
### Fixed
- `on_reconnect` now only fires after a successful message is received, not on every reconnect attempt.
- Fixed duplicate establish packets on first run, preventing multiple channels from using the same channel signal.


## [0.9.6] - 2026-01-30
### Added
- **set_on_reconnect**: Allows you to specify a callback to be executed when the connection is reestablished.
- **set_on_connect**: Allows you to specify a callback to be executed when the connection is first established.
Example:
```rust
#[cfg(feature = "hydrate")]
{
    use leptos_ws::ServerSignalWebSocket;
    let context = expect_context::<ServerSignalWebSocket>();
    context.set_on_reconnect(move || {
        leptos::logging::error!("WebSocket disconnected");
        // Handle reconnect event
    });
}
```

## [0.9.5] - 2026-01-30
### Added
#### Reconnection Handling
- Implemented automatic reconnection after connection loss, with a 1-second delay before each attempt.
- When a disconnect is detected, the provided `on_disconnect` callback will be invoked.
- **set_on_disconnect**: Allows you to specify a callback to be executed when the connection is lost.
Example:
```rust
#[cfg(feature = "hydrate")]
{
    use leptos_ws::ServerSignalWebSocket;
    let context = expect_context::<ServerSignalWebSocket>();
    context.set_on_disconnect(move || {
        leptos::logging::error!("WebSocket disconnected");
        // Handle disconnect event
    });
}
```

## [0.9.4] - 2026-01-19
### Added
- **new_with_context**: Added to address instantiation of Leptos_ws signals/channels outside Leptos context, for example in Axum handlers or Background threads

## [0.9.0] - 2025-09-09

### Added
- **BroadcastChannels**: Introduced `ChannelSignal` for broadcasting messages to multiple clients.
- **Bidirectional Signals**: Added `BiDirectionalSignal` for real-time, two-way synchronization between client and server.

### Changed
- Switched internal storage to use `DashMap` for improved concurrency and performance.
- Switched to using Leptos native websockets for communication.
- Refactored and clarified feature flags and documentation in the README.
- Expanded README usage section to include both client and server-side code in a single example.

## [0.8.0] - 2025-09-06

### Changed
- Now support leptos 0.8.0


## [0.7.8] - 2024-03-25

### Changed
- Now support leptos 0.7.8
- Changed codee to 0.3


## [0.7.7] - 2024-03-02

### Changed
- Now support leptos 0.7.7

## [0.7.0-rc1] - 2024-11-16

### Changed
- Now support rc of leptos

### Fixed
- Fixed Issues with Reconnects

## [0.7.0-beta5] - 2024-09-28

### Changed
- Now support beta5 of leptos

### Fixed
- Fixed Issues with Hydration

## [0.7.0-beta4.1] - 2024-09-02

### Changed

- Use [leptos-use](https://leptos-use.rs/) instead of own client websocket implementation
