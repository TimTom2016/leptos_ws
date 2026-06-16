use crate::messages::{Messages, ServerSignalMessage};
use crate::traits::{WsSignalCore, private};
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use json_patch::Patch;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::Any,
    ops::Deref,
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
pub struct ClientReadOnlySignal<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
}

#[async_trait]
impl<T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static> WsSignalCore
    for ClientReadOnlySignal<T>
{
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn json(&self) -> Result<Value, Error> {
        self.json_value
            .read()
            .map(|value| value.clone())
            .map_err(|_| Error::AddingSignalFailed)
    }

    async fn update_json(&self, patch: &Patch, _id: Option<String>) -> Result<(), Error> {
        let mut writer = self
            .json_value
            .write()
            .map_err(|_| Error::UpdateSignalFailed)?;
        if json_patch::patch(&mut writer, patch).is_ok() {
            let writer_clone = writer.clone();
            drop(writer);
            self.value
                .set(serde_json::from_value(writer_clone).map_err(Error::SerializationFailed)?);
            Ok(())
        } else {
            Err(Error::UpdateSignalFailed)
        }
    }
    fn set_json(&self, new_value: Value) -> Result<(), Error> {
        let mut writer = self
            .json_value
            .write()
            .map_err(|_| Error::UpdateSignalFailed)?;
        *writer = new_value;
        let writer_clone = writer.clone();
        drop(writer);
        self.value
            .set(serde_json::from_value(writer_clone).map_err(Error::SerializationFailed)?);
        Ok(())
    }
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Err(Error::NotAvailableOnSignal)
    }

    fn on_reconnect_message(&self) -> Result<Messages, Error> {
        Ok(Messages::ServerSignal(ServerSignalMessage::Establish(
            self.name.clone(),
        )))
    }
}
impl<T> ClientReadOnlySignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str, value: T) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if let Some(signal) = signals.get_signal(name) {
            return Ok(signal);
        }

        let new_signal = Self {
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(
                serde_json::to_value(value).map_err(Error::SerializationFailed)?,
            )),
            name: name.to_owned(),
        };
        let signal = new_signal.clone();
        signals
            .create_signal(
                name,
                new_signal,
                &Messages::ServerSignal(ServerSignalMessage::Establish(name.to_owned())),
            )
            .unwrap();
        Ok(signal)
    }
}

impl<T> Deref for ClientReadOnlySignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target = ArcRwSignal<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> private::DeleteTrait for ClientReadOnlySignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn delete(&self) -> Result<(), Error> {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        if let Some(ws) = use_context::<crate::ServerSignalWebSocket>() {
            ws.send(&Messages::ServerSignal(ServerSignalMessage::Delete(
                self.name.clone(),
            )))?;
        }
        Ok(())
    }
}
