mod client;
#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub type BidirectionalSignal<T> = server::ServerReadOnlySignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type BidirectionalSignal<T> = client::ClientReadOnlySignal<T>;
