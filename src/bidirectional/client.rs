use crate::messages::SignalUpdate;
use crate::traits::WsSignalCore;
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use json_patch::Patch;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::Any,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
pub struct ClientReadOnlySignal<T>
where
    T: Clone + Send + Sync,
{
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
}

#[async_trait]
impl<T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static> WsSignalCore
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
        if json_patch::patch(writer.deref_mut(), patch).is_ok() {
            self.value.set(
                serde_json::from_value(writer.clone())
                    .map_err(|err| Error::SerializationFailed(err))?,
            );
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
        self.value.set(
            serde_json::from_value(writer.clone())
                .map_err(|err| Error::SerializationFailed(err))?,
        );
        Ok(())
    }
    #[cfg(feature = "ssr")]
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, SignalUpdate)>, Error> {
        Err(Error::NotAvailableOnSignal)
    }
}
impl<T> ClientReadOnlySignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str, value: T) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if signals.contains(&name) {
            return Ok(signals
                .get_signal::<ClientReadOnlySignal<T>>(&name)
                .unwrap());
        }
        let new_signal = Self {
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(
                serde_json::to_value(value).map_err(|err| Error::SerializationFailed(err))?,
            )),
            name: name.to_owned(),
        };
        let signal = new_signal.clone();
        signals.create_signal(name, new_signal).unwrap();
        Ok(signal)
    }
}

impl<T> Update for ClientReadOnlySignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_maybe_update<U>(&self, _fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        None
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
