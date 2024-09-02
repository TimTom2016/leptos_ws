#![doc = include_str!("../README.md")]
#![feature(unboxed_closures)]
#[cfg(not(feature = "ssr"))]
use crate::client_signal::ClientSignal;
#[cfg(not(feature = "ssr"))]
use client_signals::ClientSignals;
use codee::string::JsonSerdeCodec;
use error::Error;
use leptos::prelude::*;
use leptos::*;
use leptos_use::core::ConnectionReadyState;
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
    send: Option<Arc<dyn Fn(&Messages) + Send + Sync + 'static>>,
    ready_state: Signal<ConnectionReadyState>,
    state_signals: ClientSignals,
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
}
#[cfg(not(feature = "ssr"))]
impl ServerSignalWebSocket {
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        if self.ready_state.get() != ConnectionReadyState::Open {
            self.delayed_msgs.lock().unwrap().push(msg.clone());
        } else {
            (self.send.as_ref().unwrap())(&msg);
        }
        Ok(())
    }
    pub fn new(url: &str) -> Self {
        use leptos::prelude::{provide_context, use_context};
        use leptos_use::{use_websocket_with_options, UseWebSocketOptions, UseWebSocketReturn};
        use prelude::warn;

        let delayed_msgs = Arc::default();
        let state_signals = ClientSignals::new();
        let mut ws = Self {
            ready_state: signal(ConnectionReadyState::Closed).0.into(),
            send: None,
            state_signals,
            delayed_msgs,
        };
        let ws2 = ws.clone();
        let ws3 = ws.clone();
        let onopen_callback = move |_| {
            if let Ok(mut delayed_msgs) = ws2.delayed_msgs.lock() {
                let mut messages = delayed_msgs.clone();
                delayed_msgs.clear();
                drop(delayed_msgs);
                for msg in messages.iter() {
                    if let Err(err) = ws2.send(&msg) {
                        eprintln!("Failed to send delayed message: {:?}", err);
                    }
                }
            }
        };
        let on_message_callback = move |msg: &Messages| match msg {
            Messages::Establish(_) => todo!(),
            Messages::EstablishResponse((name, value)) => {
                ws3.state_signals.set_json(name, value.to_owned());
            }
            Messages::Update(update) => {
                let name = &update.name;
                ws3.state_signals.update(&name, update.to_owned());
            }
        };
        let UseWebSocketReturn {
            ready_state, send, ..
        } = use_websocket_with_options::<Messages, Messages, JsonSerdeCodec>(
            url,
            UseWebSocketOptions::default()
                .on_open(onopen_callback)
                .on_message(on_message_callback),
        );
        ws.ready_state = ready_state;
        ws.send = Some(Arc::new(send));
        provide_context(ws.state_signals.clone());

        ws
    }
}

#[cfg(not(feature = "ssr"))]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<(), Error> {
    use error::Error;
    use leptos::prelude::{provide_context, use_context};
    use leptos_use::{use_websocket_with_options, UseWebSocketOptions, UseWebSocketReturn};
    use prelude::warn;

    if let None = use_context::<ServerSignalWebSocket>() {
        provide_context(ServerSignalWebSocket::new(url));
        return Ok(());
    } else {
        return Ok(());
    }
}

#[cfg(feature = "ssr")]
#[inline]
fn provide_websocket_inner(_url: &str) -> Result<(), Error> {
    Ok(())
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
/// - `Ok(Some(WebSocket))` if the connection is successfully established (client-side only).
/// - `Ok(None)` if running in SSR mode.
/// - `Err(JsValue)` if there's an error establishing the connection.
///
/// # Features
///
/// - When the "ssr" feature is not enabled (client-side):
///   - Creates a new WebSocket connection.
///   - Sets up message handling for server signals.
///   - Provides context for `ServerSignalWebSocket` and `ClientSignals`.
///
/// - When the "ssr" feature is enabled (server-side):
///   - Returns `Ok(None)` without establishing a connection.
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
pub fn provide_websocket(url: &str) -> Result<(), Error> {
    provide_websocket_inner(url)
}
