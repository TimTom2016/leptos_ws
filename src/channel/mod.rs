mod client;
#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub type ChannelSignal<T> = server::ServerReadOnlySignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type ChannelSignal<T> = client::ClientReadOnlySignal<T>;
