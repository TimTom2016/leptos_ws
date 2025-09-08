use crate::messages::{BiDirectionalMessage, Messages, ServerSignalMessage, SignalUpdate};
use crate::traits::WsSignalCore;
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use futures::executor::block_on;
use json_patch::Patch;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::Any,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast::{channel, Sender};

#[derive(Clone, Debug)]
pub struct ClientBidirectionalSignal<T>
where
    T: Clone + Send + Sync,
{
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
    observers: Arc<Sender<(Option<String>, SignalUpdate)>>,
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
        if json_patch::patch(writer.deref_mut(), patch).is_ok() {
            self.value.set(
                serde_json::from_value(writer.clone())
                    .map_err(|err| Error::SerializationFailed(err))?,
            );
            if id.is_none() {
                let _ = self
                    .observers
                    .send((None, SignalUpdate::new_from_patch(self.name.clone(), patch)));
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
        self.value.set(
            serde_json::from_value(writer.clone())
                .map_err(|err| Error::SerializationFailed(err))?,
        );
        Ok(())
    }
    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, SignalUpdate)>, Error> {
        Ok(self.observers.subscribe())
    }
}
impl<T> ClientBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str, value: T) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if signals.contains(&name) {
            return Ok(signals
                .get_signal::<ClientBidirectionalSignal<T>>(&name)
                .unwrap());
        }
        let (send, _) = channel(32);
        let new_signal = Self {
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(
                serde_json::to_value(value).map_err(|err| Error::SerializationFailed(err))?,
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
        let Ok(json) = self.json_value.read() else {
            return Err(Error::UpdateSignalFailed);
        };

        let new_json = serde_json::to_value(self.value.get())?;
        let mut res = Err(Error::UpdateSignalFailed);
        if *json != new_json {
            let patch = json_patch::diff(&json, &new_json);
            drop(json);
            res = self.update_json(&patch, None).await;
        }
        res
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
