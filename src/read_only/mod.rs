mod client;
#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub type ReadOnlySignal<T> = server::ServerReadOnlySignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type ReadOnlySignal<T> = client::ClientReadOnlySignal<T>;
