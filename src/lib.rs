#![doc = include_str!("../README.md")]
// #![feature(unboxed_closures)]
#[cfg(not(feature = "ssr"))]
use crate::client_signal::ClientSignal;
use crate::messages::ServerSignalMessage;
#[cfg(not(feature = "ssr"))]
use client_signals::ClientSignals;
use codee::string::JsonSerdeCodec;
use leptos::prelude::*;
#[cfg(not(feature = "ssr"))]
use leptos_use::core::ConnectionReadyState;
#[cfg(not(feature = "ssr"))]
use leptos_use::{use_websocket_with_options, UseWebSocketOptions, UseWebSocketReturn};
#[cfg(not(feature = "ssr"))]
use messages::Messages;
#[cfg(not(feature = "ssr"))]
use std::sync::{Arc, Mutex};

pub mod error;
pub mod messages;
#[cfg(feature = "ssr")]
mod server_signal;

#[cfg(feature = "ssr")]
pub mod server_signals;

#[cfg(not(feature = "ssr"))]
mod client_signal;

#[cfg(not(feature = "ssr"))]
mod client_signals;

#[cfg(all(feature = "axum", feature = "ssr"))]
pub mod axum;

/// A type alias for a signal that synchronizes with the server.
///
/// `ServerSignal<T>` represents a reactive value that can be updated from the server
/// and reflected in the client-side UI. The actual implementation differs based on
/// whether the code is running on the server or the client.
///
/// # Type Parameters
///
/// * `T`: The type of value stored in the signal. This type must satisfy the following trait bounds:
///   - `serde::Serialize`: For serialization when sending updates across the network.
///   - `serde::Deserialize<'static>`: For deserialization when receiving updates.
///   - `Clone`: To allow the value to be cloned when necessary.
///   - `Send`: To ensure the value can be safely transferred across thread boundaries.
///   - `Sync`: To allow the value to be safely shared between threads.
///   These bounds ensure proper serialization, thread safety, and efficient handling of the signal's value.
/// # Features
///
/// This type alias is conditionally defined based on the "ssr" feature flag:
///
/// - When the "ssr" feature is enabled (server-side rendering):
///   `ServerSignal<T>` is an alias for `server_signal::ServerSignal<T>`, which is the
///   server-side implementation capable of sending updates to connected clients.
///
/// - When the "ssr" feature is not enabled (client-side):
///   `ServerSignal<T>` is an alias for `ClientSignal<T>`, which is the client-side
///   implementation that receives updates from the server.
///
/// # Usage
///
/// On the server:
/// ```rust,ignore
/// #[cfg(feature = "ssr")]
/// fn create_server_signal() -> ServerSignal<i32> {
///     ServerSignal::new("counter".to_string(), 0)
/// }
/// ```
///
/// On the client:
/// ```rust,ignore
/// #[cfg(not(feature = "ssr"))]
/// fn use_server_signal() {
///     let counter = ServerSignal::<i32>::new("counter".to_string(), 0);
///     // Use `counter.get()` to read the current value
/// }
/// ```
///
/// # Note
///
/// When using `ServerSignal`, ensure that you've set up the WebSocket connection
/// using the `provide_websocket` function in your application's root component.
#[cfg(feature = "ssr")]
pub type ServerSignal<T> = server_signal::ServerSignal<T>;
#[cfg(not(feature = "ssr"))]
pub type ServerSignal<T> = ClientSignal<T>;

#[cfg(not(feature = "ssr"))]
#[derive(Clone)]
struct ServerSignalWebSocket {
    send: Arc<dyn Fn(&Messages) + Send + Sync + 'static>,
    ready_state: Signal<ConnectionReadyState>,
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
}
#[cfg(not(feature = "ssr"))]
impl ServerSignalWebSocket {
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        if self.ready_state.get() != ConnectionReadyState::Open {
            self.delayed_msgs
                .lock()
                .expect("Failed to lock delayed_msgs")
                .push(msg.clone());
        } else {
            (self.send)(&msg);
        }
        Ok(())
    }
    pub fn new(url: &str) -> Self {
        let delayed_msgs = Arc::default();
        let state_signals = ClientSignals::new();
        let initial_connection = RwSignal::new(true);
        // Create WebSocket with custom message handler
        let UseWebSocketReturn {
            ready_state,
            send,
            open,
            ..
        } = use_websocket_with_options::<Messages, Messages, JsonSerdeCodec,_, _>(
            url,
            UseWebSocketOptions::default()
                .on_message(Self::handle_message(state_signals.clone()))
                .on_open({
                    let signals = state_signals.clone();
                    move |_| {
                        // Only reconnect if this is not the initial connection
                        if !initial_connection.get() {
                            signals.reconnect().ok();
                        }
                        initial_connection.set(false);
                    }
                })
                .immediate(false),
        );

        let ws_client = Self {
            ready_state: ready_state.clone(),
            send: Arc::new(send),
            delayed_msgs,
        };
        // Start Websocket
        open();

        // Provide ClientSignals for Child Components to work
        provide_context(state_signals);

        Self::setup_delayed_message_processor(&ws_client, ready_state);

        ws_client
    }

    fn handle_message(state_signals: ClientSignals) -> impl Fn(&Messages) {
        move |msg: &Messages| match msg {
            Messages::ServerSignal(server_msg) => match server_msg {
                ServerSignalMessage::Establish(_) => {
                    // Usually client-to-server message, ignore if received
                }
                ServerSignalMessage::EstablishResponse((name, value)) => {
                    state_signals.set_json(name, value.to_owned());
                }
                ServerSignalMessage::Update(update) => {
                    state_signals.update(&update.name, update.to_owned());
                }
            },
        }
    }

    fn setup_delayed_message_processor(
        ws_client: &Self,
        ready_state: Signal<ConnectionReadyState>,
    ) {
        let ws_clone = ws_client.clone();
        Effect::new(move |_| {
            if ready_state.get() == ConnectionReadyState::Open {
                Self::process_delayed_messages(&ws_clone);
            }
        });
    }

    fn process_delayed_messages(ws: &Self) {
        let messages = {
            let mut delayed_msgs = ws.delayed_msgs.lock().expect("Failed to lock delayed_msgs");
            delayed_msgs.drain(..).collect::<Vec<_>>()
        };

        for msg in messages {
            if let Err(err) = ws.send(&msg) {
                eprintln!("Failed to send delayed message: {:?}", err);
            }
        }
    }
}

#[cfg(not(feature = "ssr"))]
#[inline]
fn provide_websocket_inner(url: &str) -> Option<()> {

    if let None = use_context::<ServerSignalWebSocket>() {
        provide_context(ServerSignalWebSocket::new(url));
    }
    Some(())
}

#[cfg(feature = "ssr")]
#[inline]
fn provide_websocket_inner(_url: &str) -> Option<()> {
    None
}
/// Establishes and provides a WebSocket connection for server signals.
///
/// This function sets up a WebSocket connection to the specified URL and provides
/// the necessary context for handling server signals. It's designed to work differently
/// based on whether server-side rendering (SSR) is enabled or not.
///
/// # Arguments
///
/// * `url` - A string slice that holds the URL of the WebSocket server to connect to.
///
/// # Returns
///
/// Returns a `Result` which is:
/// - `Some(())` if the connection is successfully established (client-side only).
/// - `None` if running in SSR mode.
///
/// # Features
///
/// - When the "ssr" feature is not enabled (client-side):
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
///
/// fn setup_websocket() {
///     if let Ok(Some(_)) = provide_websocket("ws://example.com/socket") {
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
pub fn provide_websocket(url: &str) -> Option<()> {
    provide_websocket_inner(url)
}
