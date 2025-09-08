mod client;
#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub type BiDirectionalSignal<T> = server::ServerBidirectionalSignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type BiDirectionalSignal<T> = client::ClientBidirectionalSignal<T>;
