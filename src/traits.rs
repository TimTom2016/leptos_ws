use crate::{channel::ChannelContext, error::Error, messages::Messages};
use async_trait::async_trait;
use json_patch::Patch;
use serde_json::Value;
use std::any::Any;

/// Core trait for server-read/write signals. Implemented internally by
/// [`ReadOnlySignal`] and [`BiDirectionalSignal`].
#[async_trait]
pub trait WsSignalCore: private::DeleteTrait {
    fn as_any(&self) -> &dyn Any;
    fn name(&self) -> &str;
    fn json(&self) -> Result<Value, Error>;

    async fn update_json(&self, patch: &Patch, id: Option<String>) -> Result<(), Error>;

    fn set_json(&self, new_value: Value) -> Result<(), Error>;
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error>;
    fn on_reconnect_message(&self) -> Result<Messages, Error>;
}

/// Internal trait for channel signals. Implemented by both
/// [`ServerChannelSignal`](crate::channel::server::ServerChannelSignal) and
/// [`ClientChannelSignal`](crate::channel::client::ClientChannelSignal).
pub trait ChannelSignalTrait: private::DeleteTrait + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;

    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error>;

    fn handle_message(
        &self,
        client_id: &str,
        state: &mut dyn Any,
        message: Value,
    ) -> Result<(), Error>;

    fn create_state(&self) -> Box<dyn Any + Send + Sync>;

    fn on_reconnect_message(&self) -> Result<Messages, Error>;
}

/// Trait for handling channel messages with mutable client context.
///
/// Implement this on a struct to receive `&mut ChannelContext` alongside the message,
/// or pass a closure `Fn(&T)` for the simple case (context is ignored).
pub trait ChannelHandler<T, S = ()>: Send + Sync + 'static {
    fn handle(&self, context: &mut ChannelContext<'_, S>, msg: &T);
}

impl<T, S, F> ChannelHandler<T, S> for F
where
    F: Fn(&T) + Send + Sync + 'static,
{
    fn handle(&self, _context: &mut ChannelContext<'_, S>, msg: &T) {
        self(msg)
    }
}

/// Trait for send mappers that filter/transform outgoing messages.
///
/// The mapper receives an immutable `&ChannelContext` (read-only per-connection state)
/// and the outgoing message. Return `Some(transformed)` to deliver, or `None` to suppress.
///
/// Closures `Fn(&ChannelContext<'_, S>, &T) -> Option<T>` implement this trait automatically.
pub trait SendMapperHandler<T, S = ()>: Send + Sync + 'static {
    fn handle(&self, context: &ChannelContext<'_, S>, msg: &T) -> Option<T>;
}

impl<T, S, F> SendMapperHandler<T, S> for F
where
    F: Fn(&ChannelContext<'_, S>, &T) -> Option<T> + Send + Sync + 'static,
{
    fn handle(&self, context: &ChannelContext<'_, S>, msg: &T) -> Option<T> {
        self(context, msg)
    }
}

pub(crate) mod private {
    use crate::error::Error;

    pub trait DeleteTrait {
        fn delete(&self) -> Result<(), Error>;
    }
}
