use crate::{channel::ChannelContext, error::Error, messages::Messages};
use async_trait::async_trait;
use json_patch::Patch;
use serde_json::Value;
use std::any::Any;
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

/// Trait for channel signals that can handle server and client-side message callbacks
#[async_trait]
pub trait ChannelSignalTrait: private::DeleteTrait + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;

    /// Subscribe to updates
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error>;
    /// Call callback function with message and optional per-connection state
    fn handle_message(&self, client_id: &str, state: &mut dyn Any, message: Value) -> Result<(), Error>;

    /// Create a new per-connection state for this channel
    fn create_state(&self) -> Box<dyn Any + Send + Sync>;

    fn on_reconnect_message(&self) -> Result<Messages, Error>;
}

/// Trait for handling channel messages with mutable client context.
/// Implement this on a struct to get `&self` + `&mut ChannelContext<'_, S>` alongside `&T`,
/// or just pass a closure `Fn(&T)` for the simple case (context is ignored).
pub trait ChannelHandler<T, S = ()>: Send + Sync + 'static {
    fn handle(&self, context: &mut ChannelContext<'_, S>, msg: &T);
}

impl<T, F> ChannelHandler<T> for F
where
    F: Fn(&T) + Send + Sync + 'static,
{
    fn handle(&self, _context: &mut ChannelContext<'_, ()>, msg: &T) {
        self(msg);
    }
}

pub(crate) mod private {
    use crate::error::Error;

    pub trait DeleteTrait {
        fn delete(&self) -> Result<(), Error>;
    }
}
