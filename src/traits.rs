use crate::{error::Error, messages::SignalUpdate};
use async_trait::async_trait;
use json_patch::Patch;
use leptos::prelude::Update;
use serde_json::Value;
use std::any::Any;
#[async_trait]
pub trait WsSignalCore {
    fn as_any(&self) -> &dyn Any;
    fn name(&self) -> &str;
    fn json(&self) -> Result<Value, Error>;

    async fn update_json(&self, patch: &Patch, id: Option<String>) -> Result<(), Error>;

    fn set_json(&self, new_value: Value) -> Result<(), Error>;
    #[cfg(feature = "ssr")]
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, SignalUpdate)>, Error>;
}
