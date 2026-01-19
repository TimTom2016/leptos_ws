mod client;
#[cfg(feature = "ssr")]
mod server;

/// `ChannelSignal<T>` represents a simple message channel for communication between the server and client.
/// It enables sending and receiving messages of type `T` in a reactive, event-driven manner.
///
/// # Type Parameters
///
/// * `T`: The type of message transmitted through the channel. This type must satisfy:
///   - `serde::Serialize`: For serialization when sending updates across the network.
///   - `serde::de::DeserializeOwned`: For deserialization when receiving updates.
///   - `Clone`: To allow the value to be cloned when necessary.
///   - `Send`: To ensure the value can be safely transferred across thread boundaries.
///   - `Sync`: To allow the value to be safely shared between threads.
///
/// # Usage
///
/// On both client and server:
/// ```rust,ignore
/// // Create a channel signal named "echo"
/// let echo_channel = ChannelSignal::<String>::new("echo").unwrap();
/// ```
/// On the server, if outside of a leptos server function context, eg in an Actix or Axum
/// handler:
/// ```rust
/// #[cfg(feature = "ssr")]
/// use leptos_ws::ChannelSignal;
///     # fn get_signals_from_actix_or_axum() -> leptos_ws::WsSignals { leptos_ws::WsSignals::new() }
///     let mut signals = get_signals_from_actix_or_axum(); // get it from app state
///     let echo_channel = ChannelSignal::<String>::new_with_context(&mut signals, "echo").unwrap();
/// ```
///
/// ```rust,ignore
/// // On the client: listen for messages from the server
/// echo_channel.on_client(move |msg: &String| {
///     // Handle incoming message
///     println!("Received: {}", msg);
/// });
///
/// // On the server: listen for messages from the client
/// echo_channel.on_server(move |msg: &String| {
///     // Echo the message back to all clients
///     echo_channel.send_message(msg.clone()).unwrap();
/// });
///
/// // To send a message from the client:
/// echo_channel.send_message("Hello!".to_string()).ok();
/// ```
///
/// # Note
///
/// When using `ChannelSignal`, ensure that you've set up the WebSocket connection
/// using the `provide_websocket` function in your application's root component.
#[cfg(feature = "ssr")]
pub type ChannelSignal<T> = server::ServerChannelSignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type ChannelSignal<T> = client::ClientChannelSignal<T>;
