mod client;
#[cfg(feature = "ssr")]
mod server;

/// `BiDirectionalSignal<T>` represents a reactive value that can be updated from both the server
/// and the client, with changes automatically synchronized between them in real time.
///
/// # Type Parameters
///
/// * `T`: The type of value stored in the signal. This type must satisfy:
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
/// // Create a bidirectional signal named "count_bi"
/// let count_bi = BiDirectionalSignal::<i32>::new("count_bi", 0).unwrap();
/// ```
/// On the server, if outside of a leptos server function context, eg in an Actix or Axum
/// handler:
/// ```rust
/// #[cfg(feature = "ssr")]
/// use leptos_ws::BiDirectionalSignal;
/// # fn get_signals_from_actix_or_axum() -> leptos_ws::WsSignals { leptos_ws::WsSignals::new() }
/// let mut signals = get_signals_from_actix_or_axum(); // get it from app state
/// let count_bi = BiDirectionalSignal::<i32>::new_with_context(&mut signals, "count_bi", 0).unwrap();
/// ```
///
/// ```rust,ignore
/// // On the client: update the value
/// count_bi.update(|value| *value += 1);
///
/// // On the server: update the value
/// count_bi.update(|value| *value += 100);
///
/// // Read the current value (on either side)
/// let current = count_bi.get();
/// ```
///
/// # Note
///
/// When using `BiDirectionalSignal`, ensure that you've set up the WebSocket connection
/// using the `provide_websocket` function in your application's root component.
#[cfg(feature = "ssr")]
pub type BiDirectionalSignal<T> = server::ServerBidirectionalSignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type BiDirectionalSignal<T> = client::ClientBidirectionalSignal<T>;
