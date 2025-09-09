mod client;
#[cfg(feature = "ssr")]
mod server;

/// `ReadOnlySignal<T>` represents a reactive value that can be updated from the server
/// and reflected in the client-side UI.
///
/// # Type Parameters
///
/// * `T`: The type of value stored in the signal. This type must satisfy the following trait bounds:
///   - `serde::Serialize`: For serialization when sending updates across the network.
///   - `serde::Deserialize<'static>`: For deserialization when receiving updates.
///   - `Clone`: To allow the value to be cloned when necessary.
///   - `Send`: To ensure the value can be safely transferred across thread boundaries.
///   - `Sync`: To allow the value to be safely shared between threads.
/// # Usage
///
/// On the server:
/// ```rust,ignore
/// #[cfg(feature = "ssr")]
/// fn create_server_signal() -> ReadOnlySignal<i32> {
///     ReadOnlySignal::new("counter", 0)
/// }
/// ```
///
/// On the client:
/// ```rust,ignore
/// #[cfg(any(feature = "csr", feature = "hydrate"))]
/// fn use_server_signal() {
///     let counter = ReadOnlySignal::<i32>::new("counter", 0);
///     // Use `counter.get()` to read the current value
/// }
/// ```
///
/// # Note
///
/// When using `ReadOnlySignal`, ensure that you've set up the WebSocket connection
/// using the `provide_websocket` function in your application's root component.
#[cfg(feature = "ssr")]
pub type ReadOnlySignal<T> = server::ServerReadOnlySignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type ReadOnlySignal<T> = client::ClientReadOnlySignal<T>;
