#![doc = include_str!("../README.md")]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

// #![feature(unboxed_closures)]
use crate::messages::ServerSignalMessage;
#[cfg(any(feature = "csr", feature = "hydrate", feature = "ssr"))]
pub use bidirectional::BiDirectionalSignal;
pub use channel::ChannelContext;
#[cfg(any(feature = "csr", feature = "hydrate", feature = "ssr"))]
pub use channel::ChannelSignal;
#[cfg(feature = "ssr")]
use dashmap::DashMap;
use leptos::{
    prelude::*,
    server_fn::{BoxedStream, Websocket, codec::JsonEncoding},
    task::spawn_local,
};
use messages::{BiDirectionalMessage, ChannelMessage, Messages};
#[cfg(any(feature = "csr", feature = "hydrate", feature = "ssr"))]
pub use read_only::ReadOnlySignal;

use std::any::Any;
use std::sync::{Arc, Mutex};
pub use ws_signals::WsSignals;
mod bidirectional;
mod channel;
pub mod error;
pub mod messages;
mod read_only;
mod ws_signals;

pub mod traits;

#[cfg(any(feature = "csr", feature = "hydrate"))]
#[derive(Clone)]
pub struct ServerSignalWebSocket {
    send: Arc<Mutex<Sender<Result<Messages, ServerFnError>>>>,
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
    on_disconnect: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
    on_reconnect: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
    on_connect: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
}
#[cfg(any(feature = "csr", feature = "hydrate"))]
impl ServerSignalWebSocket {
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        // Try to send the message immediately. If the send fails (channel closed or full),
        // push it onto the delayed queue to be flushed when a reconnect succeeds.
        let cloned = msg.to_owned();
        if let Ok(mut lock) = self.send.lock() {
            if lock.try_send(Ok(cloned)).is_err() {
                // queue for later
                if let Ok(mut delayed) = self.delayed_msgs.lock() {
                    delayed.push(msg.to_owned());
                }
            }
        } else {
            // couldn't lock send - queue the message
            if let Ok(mut delayed) = self.delayed_msgs.lock() {
                delayed.push(msg.to_owned());
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a callback to be called when the websocket connection is lost.
    /// # Panics
    /// Panics if the lock is poisoned.
    pub fn set_on_disconnect(&self, on_disconnect: impl Fn() + Send + Sync + 'static) {
        *self.on_disconnect.lock().expect("poisoned lock") = Some(Box::new(on_disconnect));
    }

    /// Set a callback to be called when the websocket connection is reestablished.
    /// # Panics
    /// Panics if the lock is poisoned.
    pub fn set_on_reconnect(&self, on_reconnect: impl Fn() + Send + Sync + 'static) {
        *self.on_reconnect.lock().expect("poisoned lock") = Some(Box::new(on_reconnect));
    }

    /// Set a callback to be called when the websocket connection is first established.
    /// # Panics
    /// Panics if the lock is poisoned.
    pub fn set_on_connect(&self, on_connect: impl Fn() + Send + Sync + 'static) {
        *self.on_connect.lock().expect("poisoned lock") = Some(Box::new(on_connect));
    }
}
#[cfg(any(feature = "csr", feature = "hydrate"))]
impl Default for ServerSignalWebSocket {
    fn default() -> Self {
        let (initial_tx, _initial_rx) = mpsc::channel(0);

        let delayed_msgs: Arc<Mutex<Vec<Messages>>> = Arc::new(Mutex::new(Vec::new()));
        let send = Arc::new(Mutex::new(initial_tx));
        let state_signals = WsSignals::new();
        let id = Arc::new(String::new());
        let on_disconnect = Arc::new(Mutex::new(None::<Box<dyn Fn() + Send + Sync + 'static>>));
        let on_reconnect = Arc::new(Mutex::new(None::<Box<dyn Fn() + Send + Sync + 'static>>));
        let on_connect = Arc::new(Mutex::new(None::<Box<dyn Fn() + Send + Sync + 'static>>));
        let first_connect = Arc::new(Mutex::new(true));
        {
            let on_disconnect = on_disconnect.clone();
            let on_reconnect = on_reconnect.clone();
            let on_connect = on_connect.clone();
            let mut state_signals = state_signals.clone();
            let delayed_msgs = delayed_msgs.clone();
            let send_arc = send.clone();
            spawn_local(async move {
                use std::time::Duration;
                loop {
                    // create a fresh channel for this connection attempt
                    let (tx, rx) = mpsc::channel(32);

                    // swap in the new sender so callers will use it
                    if let Ok(mut guard) = send_arc.lock() {
                        *guard = tx.clone();
                    }

                    match leptos_ws_websocket(rx.into()).await {
                        Ok(mut messages) => {
                            // flush any delayed messages onto the new sender
                            if let Ok(mut delayed) = delayed_msgs.lock() {
                                for msg in delayed.drain(..) {
                                    // ignore errors here; if it fails, re-queue below on next loop
                                    let _ = tx.clone().try_send(Ok(msg));
                                }
                            }

                            let is_first_connect = {
                                let mut first = first_connect.lock().expect("poisoned lock");
                                let was_first = *first;
                                *first = false;
                                was_first
                            };

                            if !is_first_connect {
                                for message in state_signals.get_reconnect_messages() {
                                    let _ = tx.clone().try_send(Ok(message));
                                }
                            }

                            // Fire appropriate connection callback
                            if is_first_connect
                                && let Some(ref on_connect) =
                                    *on_connect.lock().expect("poisoned lock")
                            {
                                on_connect();
                            }

                            let mut first_message_received = false;
                            while let Some(msg) = messages.next().await {
                                let Ok(msg) = msg else {
                                    continue;
                                };

                                // Fire on_reconnect after first successful message (confirms connection is working)
                                if !first_message_received && !is_first_connect {
                                    if let Some(ref on_reconnect) =
                                        *on_reconnect.lock().expect("poisoned lock")
                                    {
                                        on_reconnect();
                                    }
                                    first_message_received = true;
                                }

                                match msg {
                                    Messages::ServerSignal(server_msg) => match server_msg {
                                        ServerSignalMessage::Establish(_) => {
                                            // Usually client-to-server message, ignore if received
                                        }
                                        ServerSignalMessage::EstablishResponse((name, value)) => {
                                            state_signals.set_json(&name, value);
                                        }
                                        ServerSignalMessage::Update(update) => {
                                            spawn_local({
                                                let state_signals = state_signals.clone();
                                                async move {
                                                    state_signals
                                                        .update(
                                                            &update.get_name().clone(),
                                                            update,
                                                            None,
                                                        )
                                                        .await;
                                                }
                                            });
                                        }
                                        ServerSignalMessage::Delete(name) => {
                                            let _ = state_signals.delete_signal(&name);
                                        }
                                    },
                                    Messages::BiDirectional(bidirectional) => match bidirectional {
                                        BiDirectionalMessage::Establish(_) => {
                                            // Usually client-to-server message, ignore if received
                                        }
                                        BiDirectionalMessage::EstablishResponse((name, value)) => {
                                            state_signals.set_json(&name, value);
                                            let recv = state_signals.add_observer(&name).unwrap();
                                            spawn_local(handle_broadcasts_client(recv, tx.clone()));
                                        }
                                        BiDirectionalMessage::Update(update) => {
                                            spawn_local({
                                                let state_signals = state_signals.clone();
                                                let id = id.clone();
                                                async move {
                                                    state_signals
                                                        .update(
                                                            &update.get_name().clone(),
                                                            update,
                                                            Some(id.to_string()),
                                                        )
                                                        .await;
                                                }
                                            });
                                        }
                                        BiDirectionalMessage::Delete(name) => {
                                            let _ = state_signals.delete_signal(&name);
                                        }
                                    },
                                    Messages::Channel(channel) => match channel {
                                        ChannelMessage::Establish(_) => {
                                            // Usually client-to-server message, ignore if received
                                        }
                                        ChannelMessage::EstablishResponse(name) => {
                                            let recv =
                                                state_signals.add_observer_channel(&name).unwrap();
                                            spawn_local(handle_broadcasts_client(recv, tx.clone()));
                                        }
                                        ChannelMessage::Message(name, value) => {
                                            state_signals.handle_message(&name, "", &mut (), value);
                                        }
                                        ChannelMessage::Delete(name) => {
                                            let _ = state_signals.delete_channel(&name);
                                        }
                                    },
                                }
                            }
                        }
                        Err(e) => leptos::logging::error!("{e}"),
                    }
                    if let Some(ref on_disconnect) = *on_disconnect.lock().expect("poisoned lock") {
                        on_disconnect();
                    }
                    // connection lost - wait and retry
                    gloo_timers::future::sleep(Duration::from_secs(1)).await;
                }
            });
        }

        let ws_client = Self {
            send,
            delayed_msgs,
            on_disconnect,
            on_reconnect,
            on_connect,
        };

        // Provide ClientSignals for Child Components to work
        provide_context(state_signals);

        ws_client
    }
}

#[cfg(any(feature = "csr", feature = "hydrate"))]
#[inline]
fn provide_websocket_inner() -> Option<()> {
    if use_context::<ServerSignalWebSocket>().is_none() {
        provide_context(ServerSignalWebSocket::new());
    }
    Some(())
}

#[allow(clippy::unused_async)]
#[server(protocol = Websocket<JsonEncoding, JsonEncoding>,endpoint="leptos_ws_websocket")]
pub async fn leptos_ws_websocket(
    input: BoxedStream<Messages, ServerFnError>,
) -> Result<BoxedStream<Messages, ServerFnError>, ServerFnError> {
    use futures::{SinkExt, StreamExt, channel::mpsc};
    let mut input = input;
    let (mut tx, rx) = mpsc::channel(1);
    let Some(server_signals) = use_context::<WsSignals>() else {
        leptos::logging::error!("WsSignals not found in context");
        return Err(ServerFnError::new("WsSignals not found in context"));
    };
    let id = Arc::new(nanoid::nanoid!());
    let tasks: Arc<DashMap<(String, String), tokio::task::AbortHandle>> = Arc::new(DashMap::new());
    tracing::info!(connection_id = %id, "new WebSocket connection established");
    // Per-channel state for this connection
    let connection_state: Arc<DashMap<String, Box<dyn Any + Send + Sync>>> = Arc::new(DashMap::new());
    // spawn a task to listen to the input stream of messages coming in over the websocket
    tokio::spawn(async move {
        while let Some(msg) = input.next().await {
            let Ok(msg) = msg else {
                break;
            };
            match msg {
                Messages::ServerSignal(server_msg) => match server_msg {
                    ServerSignalMessage::Establish(name) => {
                        tracing::debug!(connection_id = %id, signal_name = %name, "client establishing server signal");
                        let Some(recv) = server_signals.add_observer(&name) else {
                            leptos::logging::error!("Signal '{}' not found", name);
                            continue;
                        };
                        let Some(Ok(value)) = server_signals.json(&name) else {
                            leptos::logging::error!("Failed to get JSON for signal '{}'", name);
                            continue;
                        };
                        if tx
                            .send(Ok(Messages::ServerSignal(
                                ServerSignalMessage::EstablishResponse((name.clone(), value)),
                            )))
                            .await
                            .is_err()
                        {
                            leptos::logging::error!("Failed to send EstablishResponse to client");
                            break;
                        }
                        let handle =
                            tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                        tasks.insert((id.to_string(), name.clone()), handle.abort_handle());
                    }
                    ServerSignalMessage::Delete(name) => {
                        tracing::debug!(connection_id = %id, signal_name = %name, "client unsubscribing from server signal");
                        if let Some(entry) = tasks.remove(&(id.to_string(), name)) {
                            entry.1.abort();
                        }
                    }
                    _ => leptos::logging::error!("Unexpected server signal message from client"),
                },
                Messages::BiDirectional(bidirectional) => match bidirectional {
                    BiDirectionalMessage::Establish(name) => {
                        tracing::debug!(connection_id = %id, signal_name = %name, "client establishing bidirectional signal");
                        let Some(recv) = server_signals.add_observer(&name) else {
                            leptos::logging::error!("Bidirectional signal '{}' not found", name);
                            continue;
                        };
                        let Some(Ok(value)) = server_signals.json(&name) else {
                            leptos::logging::error!(
                                "Failed to get JSON for bidirectional signal '{}'",
                                name
                            );
                            continue;
                        };
                        if tx
                            .send(Ok(Messages::BiDirectional(
                                BiDirectionalMessage::EstablishResponse((name.clone(), value)),
                            )))
                            .await
                            .is_err()
                        {
                            leptos::logging::error!("Failed to send EstablishResponse to client");
                            break;
                        }
                        let handle =
                            tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                        tasks.insert((id.to_string(), name.clone()), handle.abort_handle());
                    }
                    BiDirectionalMessage::Update(update) => {
                        tracing::debug!(connection_id = %id, signal_name = %update.get_name().clone(), "client sent bidirectional update");
                        server_signals
                            .update(&update.get_name().clone(), update, Some(id.to_string()))
                            .await;
                    }
                    BiDirectionalMessage::Delete(name) => {
                        tracing::debug!(connection_id = %id, signal_name = %name, "client unsubscribing from bidirectional signal");
                        if let Some(entry) = tasks.remove(&(id.to_string(), name)) {
                            entry.1.abort();
                        }
                    }
                    _ => leptos::logging::error!("Unexpected bi-directional message from client"),
                },
                Messages::Channel(channel) => match channel {
                    ChannelMessage::Establish(name) => {
                        tracing::debug!(connection_id = %id, channel_name = %name, "client establishing channel");
                        let Some(recv) = server_signals.add_observer_channel(&name) else {
                            leptos::logging::error!("Channel '{}' not found", name);
                            continue;
                        };
                        // Create per-connection state for this channel
                        if let Some(state) = server_signals.create_channel_state(&name) {
                            connection_state.insert(name.clone(), state);
                        }
                        if tx
                            .send(Ok(Messages::Channel(ChannelMessage::EstablishResponse(
                                name.clone(),
                            ))))
                            .await
                            .is_err()
                        {
                            leptos::logging::error!("Failed to send EstablishResponse to client");
                            break;
                        }
                        let handle =
                            tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                        tasks.insert((id.to_string(), name.clone()), handle.abort_handle());
                    }

                    ChannelMessage::Message(name, value) => {
                        tracing::debug!(connection_id = %id, channel_name = %name, "client sent channel message");
                        if let Some(mut state) = connection_state.get_mut(&name) {
                            server_signals.handle_message(&name, &id, &mut *state, value);
                        }
                    }
                    ChannelMessage::Delete(name) => {
                        tracing::debug!(connection_id = %id, channel_name = %name, "client unsubscribing from channel");
                        connection_state.remove(&name);
                        if let Some(entry) = tasks.remove(&(id.to_string(), name)) {
                            entry.1.abort();
                        }
                    }
                    _ => leptos::logging::error!("Unexpected channel message from client"),
                },
            }
        }
        tracing::info!(connection_id = %id, "WebSocket client disconnected");
    });

    Ok(rx.into())
}
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{self, Sender},
};

#[cfg(any(feature = "csr", feature = "hydrate"))]
async fn handle_broadcasts_client(
    mut receiver: tokio::sync::broadcast::Receiver<(Option<String>, Messages)>,
    mut sink: Sender<Result<Messages, ServerFnError>>,
) {
    loop {
        match receiver.recv().await {
            Ok(message) => {
                if sink.send(Ok(message.1)).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => (),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(feature = "ssr")]
async fn handle_broadcasts(
    id: String,
    mut receiver: tokio::sync::broadcast::Receiver<(Option<String>, Messages)>,
    mut sink: Sender<Result<Messages, ServerFnError>>,
) {
    loop {
        match receiver.recv().await {
            Ok(message) => {
                if message.0.is_some_and(|v| id == v) {
                    tracing::debug!(connection_id = %id, "skipping broadcast from self");
                    continue;
                }
                tracing::trace!(connection_id = %id, "broadcasting message to client");
                if sink.send(Ok(message.1)).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(connection_id = %id, lagged = n, "broadcast receiver lagged");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(all(feature = "ssr", not(any(feature = "hydrate", feature = "csr"))))]
#[inline]
fn provide_websocket_inner() -> Option<()> {
    None
}
/// Establishes and provides a WebSocket connection for server signals.
///
/// This function sets up a WebSocket connection to the specified URL and provides
/// the necessary context for handling server signals. It's designed to work differently
/// based on whether server-side rendering (SSR) is enabled or the "hydrate" feature is enabled.
///
/// # Returns
///
/// Returns a `Result` which is:
/// - `Some(())` if the connection is successfully established (client-side only).
/// - `None` if running in SSR mode.
///
/// # Features
///
/// - When the "hydrate" feature is enabled (client-side):
///   - Creates a new WebSocket connection.
///   - Sets up message handling for server signals.
///   - Provides context for `ServerSignalWebSocket` and `ClientSignals`.
///
/// - When the "ssr" feature is enabled (server-side):
///   - Returns `None` without establishing a connection.
///
/// # Examples
///
/// ```rust
/// use leptos_ws::provide_websocket;
/// fn setup_websocket() {
///     if let Some(_) = provide_websocket() {
///         println!("WebSocket connection established");
///     } else {
///         println!("Running in SSR mode or connection failed");
///     }
/// }
/// ```
///
/// # Note
///
/// This function should be called in the root component of your Leptos application
/// to ensure the WebSocket connection is available throughout the app.
#[cfg(any(feature = "csr", feature = "hydrate", feature = "ssr"))]
pub fn provide_websocket() -> Option<()> {
    provide_websocket_inner()
}
