#![doc = include_str!("../README.md")]
// #![feature(unboxed_closures)]
#[cfg(any(feature = "csr", feature = "hydrate"))]
use crate::client_signal::ClientSignal;
use crate::messages::ServerSignalMessage;
#[cfg(any(feature = "csr", feature = "hydrate"))]
use client_signals::ClientSignals;
use codee::string::JsonSerdeCodec;
use leptos::{
    prelude::*,
    server_fn::{codec::JsonEncoding, BoxedStream, Websocket},
    task::spawn_local,
};
use messages::Messages;
#[cfg(any(feature = "csr", feature = "hydrate"))]
use std::sync::{Arc, Mutex};
pub mod error;
pub mod messages;
#[cfg(feature = "ssr")]
mod server_signal;

#[cfg(feature = "ssr")]
pub mod server_signals;

#[cfg(any(feature = "csr", feature = "hydrate"))]
mod client_signal;

#[cfg(any(feature = "csr", feature = "hydrate"))]
mod client_signals;

use std::future::Future;
use std::pin::Pin;

// Type alias for the websocket function signature
type WebsocketFn = Box<
    dyn Fn(
            BoxedStream<Messages, ServerFnError>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<BoxedStream<Messages, ServerFnError>, ServerFnError>>
                    + Send,
            >,
        > + Send
        + 'static,
>;

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
/// #[cfg(any(feature = "csr", feature = "hydrate"))]
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
#[cfg(any(feature = "csr", feature = "hydrate"))]
pub type ServerSignal<T> = ClientSignal<T>;

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
    pub fn new<F, Fut>(websocket_fn: F) -> Self
    where
        F: Fn(BoxedStream<Messages, ServerFnError>) -> Fut + Clone + Send + Sync + 'static,
        Fut: std::future::Future<
                Output = Result<BoxedStream<Messages, ServerFnError>, ServerFnError>,
            > + Send
            + 'static,
    {
        let (tx, rx) = mpsc::channel(32);

        let delayed_msgs = Arc::default();
        let state_signals = ClientSignals::new();
        spawn_local({
            let state_signals = state_signals.clone();
            async move {
                match websocket_fn(rx.into()).await {
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
                                        state_signals.update(&update.name, update.to_owned());
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
fn provide_websocket_inner<F, Fut>(websocket_fn: F) -> Option<()>
where
    F: Fn(BoxedStream<Messages, ServerFnError>) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<BoxedStream<Messages, ServerFnError>, ServerFnError>>
        + Send
        + 'static,
{
    if let None = use_context::<ServerSignalWebSocket>() {
        provide_context(ServerSignalWebSocket::new(Box::new(websocket_fn)));
    }
    Some(())
}
#[cfg(feature = "ssr")]
pub async fn leptos_ws_websocket_inner(
    input: BoxedStream<Messages, ServerFnError>,
) -> Result<BoxedStream<Messages, ServerFnError>, ServerFnError> {
    use futures::{channel::mpsc, SinkExt, StreamExt};
    let mut input = input;
    let (mut tx, rx) = mpsc::channel(1);
    let server_signals = use_context::<crate::server_signals::ServerSignals>().unwrap();
    // spawn a task to listen to the input stream of messages coming in over the websocket
    tokio::spawn(async move {
        let mut x = 0;
        while let Some(msg) = input.next().await {
            let Ok(msg) = msg else {
                break;
            };
            match msg {
                Messages::ServerSignal(server_msg) => match server_msg {
                    ServerSignalMessage::Establish(name) => {
                        let recv = server_signals.add_observer(name.clone()).await.unwrap();
                        tx.send(Ok(Messages::ServerSignal(
                            ServerSignalMessage::EstablishResponse((
                                name.clone(),
                                server_signals.json(name.clone()).await.unwrap().unwrap(),
                            )),
                        )))
                        .await
                        .unwrap();
                        tokio::spawn(handle_broadcasts(recv, tx.clone()));
                    }
                    _ => leptos::logging::error!("Unexpected server signal message from client"),
                },
            }
        }
    });

    Ok(rx.into())
}
use futures::{
    channel::mpsc::{self, Receiver, Sender},
    SinkExt, StreamExt,
};
#[cfg(feature = "ssr")]
use messages::ServerSignalUpdate;

#[cfg(feature = "ssr")]
async fn handle_broadcasts(
    mut receiver: tokio::sync::broadcast::Receiver<ServerSignalUpdate>,
    mut sink: Sender<Result<Messages, ServerFnError>>,
) {
    use futures::{SinkExt, StreamExt};

    while let Ok(message) = receiver.recv().await {
        if sink
            .send(Ok(Messages::ServerSignal(ServerSignalMessage::Update(
                message,
            ))))
            .await
            .is_err()
        {
            break;
        };
    }
}

#[cfg(feature = "ssr")]
#[inline]
fn provide_websocket_inner<F, Fut>(websocket_fn: F) -> Option<()>
where
    F: Fn(BoxedStream<Messages, ServerFnError>) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<BoxedStream<Messages, ServerFnError>, ServerFnError>>
        + Send
        + 'static,
{
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
///     if let Some(_) = provide_websocket("ws://example.com/socket") {
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
pub fn provide_websocket<F, Fut>(websocket_fn: F) -> Option<()>
where
    F: Fn(BoxedStream<Messages, ServerFnError>) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<BoxedStream<Messages, ServerFnError>, ServerFnError>>
        + Send
        + 'static,
{
    provide_websocket_inner(websocket_fn)
}
