mod client;
#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub type ChannelSignal<T> = server::ServerChannelSignal<T>;
#[cfg(all(any(feature = "csr", feature = "hydrate"), not(feature = "ssr")))]
pub type ChannelSignal<T> = client::ClientChannelSignal<T>;
