use std::any::Any;
use std::ops::Deref;
use std::panic::Location;
use std::sync::{Arc, RwLock};

use crate::error::Error;
use crate::messages::{BiDirectionalMessage, Messages, SignalUpdate};
use crate::traits::WsSignalCore;
use crate::ws_signals::WsSignals;
use async_trait::async_trait;
use futures::executor::block_on;
use guards::{Plain, ReadGuard};
use json_patch::Patch;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::{channel, Sender};

/// A signal owned by the server which writes to the websocket when mutated.
#[derive(Clone, Debug)]
pub struct ServerBidirectionalSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de>,
{
    initial: T,
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
    observers: Arc<Sender<(Option<String>, Messages)>>,
}
#[async_trait]
impl<T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static> WsSignalCore
    for ServerBidirectionalSignal<T>
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
        let mut writer = self.json_value.write();

        let Ok(mut writer) = writer.as_deref_mut() else {
            return Err(Error::UpdateSignalFailed);
        };

        if json_patch::patch(&mut writer, patch).is_ok() {
            if id.is_some() {
                self.value.set(
                    serde_json::from_value(writer.clone())
                        .map_err(|err| Error::SerializationFailed(err))?,
                );
            }
            let _ = self.observers.send((
                id,
                Messages::BiDirectional(BiDirectionalMessage::Update(
                    SignalUpdate::new_from_patch(self.name.clone(), patch),
                )),
            ));
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
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Ok(self.observers.subscribe())
    }
}

impl<T> ServerBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str, value: T) -> Result<Self, Error> {
        let mut signals = use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if signals.contains(&name) {
            return Ok(signals
                .get_signal::<ServerBidirectionalSignal<T>>(name)
                .unwrap());
        }
        let (send, _) = channel(32);
        let new_signal = ServerBidirectionalSignal {
            initial: value.clone(),
            name: name.to_owned(),
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(serde_json::to_value(value)?)),
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
    fn check_is_hydrating(&self) -> bool {
        #[cfg(feature = "ssr")]
        {
            let owner = match Owner::current() {
                Some(owner) => owner,
                None => return false,
            };
            let shared_context = match owner.shared_context() {
                Some(shared_context) => shared_context,
                None => return false,
            };
            return shared_context.get_is_hydrating() || !shared_context.during_hydration();
        }
        #[allow(unreachable_code)]
        false
    }
}

impl<T> Update for ServerBidirectionalSignal<T>
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

impl<T> DefinedAt for ServerBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.value.defined_at()
    }
}

impl<T> ReadUntracked for ServerBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = ReadGuard<T, Plain<T>>;

    fn try_read_untracked(&self) -> Option<Self::Value> {
        if self.check_is_hydrating() {
            let guard: ReadGuard<T, Plain<T>> = ReadGuard::new(
                Plain::try_new(Arc::new(std::sync::RwLock::new(self.initial.clone()))).unwrap(),
            );
            return Some(guard);
        }

        self.value.try_read_untracked()
    }
}

impl<T> Get for ServerBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_get(&self) -> Option<Self::Value> {
        #[cfg(feature = "ssr")]
        if self.check_is_hydrating() {
            return Some(self.initial.clone());
        }
        self.value.try_get()
    }
}

impl<T> Deref for ServerBidirectionalSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target = ArcRwSignal<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
