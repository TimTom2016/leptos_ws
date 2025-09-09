#![doc = include_str!("../README.md")]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

// #![feature(unboxed_closures)]
use crate::messages::ServerSignalMessage;
pub use bidirectional::BiDirectionalSignal;
pub use channel::ChannelSignal;
use leptos::{
    prelude::*,
    server_fn::{BoxedStream, Websocket, codec::JsonEncoding},
    task::spawn_local,
};
use messages::{BiDirectionalMessage, ChannelMessage, Messages};
pub use read_only::ReadOnlySignal;

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
struct ServerSignalWebSocket {
    send: Sender<Result<Messages, ServerFnError>>,
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
}
#[cfg(any(feature = "csr", feature = "hydrate"))]
impl ServerSignalWebSocket {
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        let mut send = self.send.clone();
        send.try_send(Ok(msg.to_owned()));
        Ok(())
    }
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);

        let delayed_msgs = Arc::default();
        let state_signals = WsSignals::new();
        let id = Arc::new(String::new());
        spawn_local({
            let state_signals = state_signals.clone();
            let tx = tx.clone();
            async move {
                match leptos_ws_websocket(rx.into()).await {
                    Ok(mut messages) => {
                        while let Some(msg) = messages.next().await {
                            let Ok(msg) = msg else {
                                leptos::logging::error!(
                                    "{}",
                                    msg.expect_err("Exepcting Error because of else unwrap")
                                );
                                continue;
                            };
                            match msg {
                                Messages::ServerSignal(server_msg) => match server_msg {
                                    ServerSignalMessage::Establish(_) => {
                                        // Usually client-to-server message, ignore if received
                                    }
                                    ServerSignalMessage::EstablishResponse((name, value)) => {
                                        state_signals.set_json(&name, value.to_owned());
                                    }
                                    ServerSignalMessage::Update(update) => {
                                        spawn_local({
                                            let state_signals = state_signals.clone();
                                            async move {
                                                state_signals
                                                    .update(
                                                        update.get_name(),
                                                        update.to_owned(),
                                                        None,
                                                    )
                                                    .await;
                                            }
                                        });
                                    }
                                },
                                Messages::BiDirectional(bidirectional) => match bidirectional {
                                    BiDirectionalMessage::Establish(_) => {
                                        // Usually client-to-server message, ignore if received
                                    }
                                    BiDirectionalMessage::EstablishResponse((name, value)) => {
                                        state_signals.set_json(&name, value.to_owned());
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
                                                        update.get_name(),
                                                        update.to_owned(),
                                                        Some(id.to_string()),
                                                    )
                                                    .await;
                                            }
                                        });
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
                                        state_signals.handle_message(&name, value);
                                    }
                                },
                            }
                        }
                    }
                    Err(e) => leptos::logging::error!("{e}"),
                }
            }
        });

        let ws_client = Self {
            send: tx,
            delayed_msgs,
        };

        // Provide ClientSignals for Child Components to work
        provide_context(state_signals);

        ws_client
    }
}

#[cfg(any(feature = "csr", feature = "hydrate"))]
#[inline]
fn provide_websocket_inner() -> Option<()> {
    if let None = use_context::<ServerSignalWebSocket>() {
        provide_context(ServerSignalWebSocket::new());
    }
    Some(())
}
#[server(protocol = Websocket<JsonEncoding, JsonEncoding>,endpoint="leptos_ws_websocket")]
pub async fn leptos_ws_websocket(
    input: BoxedStream<Messages, ServerFnError>,
) -> Result<BoxedStream<Messages, ServerFnError>, ServerFnError> {
    use futures::{SinkExt, StreamExt, channel::mpsc};
    let mut input = input;
    let (mut tx, rx) = mpsc::channel(1);
    let server_signals = use_context::<WsSignals>().unwrap();
    let id = Arc::new(nanoid::nanoid!());
    // spawn a task to listen to the input stream of messages coming in over the websocket
    tokio::spawn(async move {
        while let Some(msg) = input.next().await {
            let Ok(msg) = msg else {
                break;
            };
            match msg {
                Messages::ServerSignal(server_msg) => match server_msg {
                    ServerSignalMessage::Establish(name) => {
                        let recv = server_signals.add_observer(&name).unwrap();
                        tx.send(Ok(Messages::ServerSignal(
                            ServerSignalMessage::EstablishResponse((
                                name.clone(),
                                server_signals.json(&name).unwrap().unwrap(),
                            )),
                        )))
                        .await
                        .unwrap();
                        tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                    }
                    _ => leptos::logging::error!("Unexpected server signal message from client"),
                },
                Messages::BiDirectional(bidirectional) => match bidirectional {
                    BiDirectionalMessage::Establish(name) => {
                        let recv = server_signals.add_observer(&name).unwrap();
                        tx.send(Ok(Messages::BiDirectional(
                            BiDirectionalMessage::EstablishResponse((
                                name.clone(),
                                server_signals.json(&name).unwrap().unwrap(),
                            )),
                        )))
                        .await
                        .unwrap();
                        tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                    }
                    BiDirectionalMessage::Update(update) => {
                        server_signals
                            .update(update.get_name(), update.to_owned(), Some(id.to_string()))
                            .await;
                    }
                    _ => leptos::logging::error!("Unexpected bi-directional message from client"),
                },
                Messages::Channel(channel) => match channel {
                    ChannelMessage::Establish(name) => {
                        let recv = server_signals.add_observer_channel(&name).unwrap();
                        tx.send(Ok(Messages::Channel(ChannelMessage::EstablishResponse(
                            name.clone(),
                        ))))
                        .await
                        .unwrap();
                        tokio::spawn(handle_broadcasts(id.to_string(), recv, tx.clone()));
                    }

                    ChannelMessage::Message(name, value) => {
                        server_signals.handle_message(&name, value);
                    }
                    _ => leptos::logging::error!("Unexpected channel message from client"),
                },
            }
        }
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
    while let Ok(message) = receiver.recv().await {
        if sink.send(Ok(message.1)).await.is_err() {
            break;
        };
    }
}

#[cfg(feature = "ssr")]
async fn handle_broadcasts(
    id: String,
    mut receiver: tokio::sync::broadcast::Receiver<(Option<String>, Messages)>,
    mut sink: Sender<Result<Messages, ServerFnError>>,
) {
    while let Ok(message) = receiver.recv().await {
        if message.0.is_some_and(|v| id == v) {
            continue;
        }
        if sink.send(Ok(message.1)).await.is_err() {
            break;
        };
    }
}

#[cfg(feature = "ssr")]
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
pub fn provide_websocket() -> Option<()> {
    provide_websocket_inner()
}
