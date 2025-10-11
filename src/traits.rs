use crate::{error::Error, messages::Messages};
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
}

/// Trait for channel signals that can handle server and client-side message callbacks
#[async_trait]
pub trait ChannelSignalTrait: private::DeleteTrait + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;

    /// Subscribe to updates
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error>;
    /// Call callback function with message
    fn handle_message(&self, message: Value) -> Result<(), Error>;
}

pub(crate) mod private {
    use crate::error::Error;

    pub trait DeleteTrait {
        fn delete(&self) -> Result<(), Error>;
    }
}
