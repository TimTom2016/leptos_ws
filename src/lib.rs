#![doc = include_str!("../README.md")]
#![feature(unboxed_closures)]
#[cfg(not(feature = "ssr"))]
use crate::client_signal::ClientSignal;
#[cfg(not(feature = "ssr"))]
use client_signals::ClientSignals;
use leptos::*;
#[cfg(not(feature = "ssr"))]
use messages::Messages;
#[cfg(not(feature = "ssr"))]
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsValue;
use web_sys::WebSocket;

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
    ws: send_wrapper::SendWrapper<WebSocket>,
    // References to these are kept by the closure for the callback
    // onmessage callback on the websocket
    state_signals: ClientSignals,
    // When the websocket is first established, the leptos may not have
    // completed the traversal that sets up all of the state signals.
    // Without that, we don't have a base state to apply the patches to,
    // and therefore we must keep a record of the patches to apply after
    // the state has been set up.
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
}
#[cfg(not(feature = "ssr"))]
impl ServerSignalWebSocket {
    /// Returns the inner websocket.
    pub fn ws(&self) -> WebSocket {
        self.ws.clone().take()
    }
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        let serialized_msg = serde_json::to_string(&msg)?;
        if self.ws.ready_state() != WebSocket::OPEN {
            self.delayed_msgs.lock().unwrap().push(msg.clone());
        } else {
            self.ws
                .send_with_str(&serialized_msg)
                .expect("Failed to send message");
        }
        Ok(())
    }
}

#[cfg(not(feature = "ssr"))]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<Option<WebSocket>, JsValue> {
    use leptos::prelude::{provide_context, use_context};
    use prelude::warn;
    use wasm_bindgen::{prelude::Closure, JsCast};
    use web_sys::js_sys::{Function, JsString};
    use web_sys::MessageEvent;

    if let None = use_context::<ServerSignalWebSocket>() {
        let ws = send_wrapper::SendWrapper::new(WebSocket::new(url)?);
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        provide_context(ServerSignalWebSocket {
            ws,
            state_signals: ClientSignals::new(),
            delayed_msgs: Arc::default(),
        });
    }

    match use_context::<ServerSignalWebSocket>() {
        Some(ws) => {
            let handlers = ws.state_signals.clone();
            provide_context(ws.state_signals.clone());

            let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
                let ws_string = event
                    .data()
                    .dyn_into::<JsString>()
                    .unwrap()
                    .as_string()
                    .unwrap();

                match serde_json::from_str::<Messages>(&ws_string) {
                    Ok(Messages::Establish(_)) => todo!(),
                    Ok(Messages::EstablishResponse((name, value))) => {
                        handlers.set_json(name, value);
                    }
                    Ok(Messages::Update(update)) => {
                        let name = &update.name;
                        handlers.update(name.to_string(), update);
                    }
                    Err(err) => {
                        warn!("Couldn't deserialize Socket Message {}", err)
                    }
                }
            }) as Box<dyn FnMut(_)>);
            let ws2 = ws.clone();
            let onopen_callback = Closure::<dyn FnMut()>::new(move || {
                if let Ok(mut delayed_msgs) = ws2.delayed_msgs.lock() {
                    for msg in delayed_msgs.drain(..) {
                        if let Err(err) = ws2.send(&msg) {
                            eprintln!("Failed to send delayed message: {:?}", err);
                        }
                    }
                }
            });
            let function: &Function = callback.as_ref().unchecked_ref();
            ws.ws.set_onmessage(Some(function));
            ws.ws
                .set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();
            // Keep the closure alive for the lifetime of the program
            callback.forget();

            Ok(Some(ws.ws()))
        }
        None => todo!(),
    }
}

#[cfg(feature = "ssr")]
#[inline]
fn provide_websocket_inner(_url: &str) -> Result<Option<WebSocket>, JsValue> {
    Ok(None)
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
pub fn provide_websocket(url: &str) -> Result<Option<WebSocket>, JsValue> {
    provide_websocket_inner(url)
}
