use crate::messages::{BiDirectionalMessage, Messages, SignalUpdate};
use crate::traits::{WsSignalCore, private};
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use futures::executor::block_on;
use json_patch::Patch;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::Any,
    ops::Deref,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast::{Sender, channel};

#[derive(Clone, Debug)]
pub struct ClientBidirectionalSignal<T>
where
    T: Clone + Send + Sync,
{
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
    observers: Arc<Sender<(Option<String>, Messages)>>,
}

#[async_trait]
impl<T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static> WsSignalCore
    for ClientBidirectionalSignal<T>
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

    async fn update_json(&self, patch: &Patch, id: Option<String>) -> Result<(), Error> {
        let mut writer = self
            .json_value
            .write()
            .map_err(|_| Error::UpdateSignalFailed)?;
        if json_patch::patch(&mut writer, patch).is_ok() {
            let writer_clone = writer.clone();
            drop(writer);
            self.value
                .set(serde_json::from_value(writer_clone).map_err(Error::SerializationFailed)?);
            if id.is_none() {
                let _ = self.observers.send((
                    None,
                    Messages::BiDirectional(BiDirectionalMessage::Update(
                        SignalUpdate::new_from_patch(self.name.clone(), patch),
                    )),
                ));
            }
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
        Ok(self.observers.subscribe())
    }

    fn on_reconnect_message(&self) -> Result<Messages, Error> {
        Ok(Messages::BiDirectional(BiDirectionalMessage::Establish(
            self.name.clone(),
        )))
    }
}
impl<T> ClientBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str, value: T) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if signals.contains(name) {
            return Ok(signals.get_signal(name).unwrap());
        }
        let (send, _) = channel(32);
        let new_signal = Self {
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(
                serde_json::to_value(value).map_err(Error::SerializationFailed)?,
            )),
            name: name.to_owned(),
            observers: Arc::new(send),
        };
        let signal = new_signal.clone();
        signals
            .create_signal(
                name,
                new_signal,
                &Messages::BiDirectional(BiDirectionalMessage::Establish(name.to_owned())),
            )
            .unwrap();

        Ok(signal)
    }

    async fn update_if_changed(&self) -> Result<(), Error> {
        let patch = {
            let json = self.json_value.read().map_err(|_| Error::UpdateSignalFailed)?;
            let new_json = serde_json::to_value(self.value.get())?;
            if *json == new_json {
                return Err(Error::UpdateSignalFailed);
            }
            let patch = json_patch::diff(&json, &new_json);
            drop(json);
            patch
        };
        self.update_json(&patch, None).await
    }
}

impl<T> Update for ClientBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        let mut lock = self.value.try_write()?;
        let (did_update, val) = fun(&mut *lock);
        if !did_update {
            lock.untrack();
        }
        drop(lock);
        block_on(async move {
            let _ = self.update_if_changed().await;
        });
        Some(val)
    }
}

impl<T> Deref for ClientBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target = ArcRwSignal<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> private::DeleteTrait for ClientBidirectionalSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn delete(&self) -> Result<(), Error> {
        Err(Error::NotAvailableOnClient)
    }
}
