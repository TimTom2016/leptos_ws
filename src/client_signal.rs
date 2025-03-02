use crate::error::Error;
use crate::{client_signals::ClientSignals, messages::ServerSignalUpdate};
use async_trait::async_trait;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::Any,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
pub struct ClientSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de>,
{
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
}

#[async_trait]
pub trait ClientSignalTrait {
    fn as_any(&self) -> &dyn Any;
    fn update_json(&self, patch: ServerSignalUpdate) -> Result<(), Error>;
    fn set_json(&self, new_value: Value) -> Result<(), Error>;
}
impl<T> ClientSignalTrait for ClientSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static + Serialize,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[track_caller]

    fn update_json(&self, patch: ServerSignalUpdate) -> Result<(), Error> {
        let mut writer = self
            .json_value
            .write()
            .map_err(|_| Error::UpdateSignalFailed)?;
        if json_patch::patch(writer.deref_mut(), &patch.patch).is_ok() {
            *self.value.write() = serde_json::from_value(writer.clone())
                .map_err(|err| Error::SerializationFailed(err))?;
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
        *self.value.write() = serde_json::from_value(writer.clone())
            .map_err(|err| Error::SerializationFailed(err))?;
        Ok(())
    }
}

impl<T> ClientSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: String, value: T) -> Result<Self, Error> {
        let mut signals: ClientSignals =
            use_context::<ClientSignals>().ok_or(Error::MissingServerSignals)?;
        if signals.contains(&name) {
            return Ok(signals.get_signal::<ClientSignal<T>>(&name).unwrap());
        }
        let new_signal = Self {
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(
                serde_json::to_value(value).map_err(|err| Error::SerializationFailed(err))?,
            )),
        };
        let signal = new_signal.clone();
        signals.create_signal(name, new_signal).unwrap();
        Ok(signal)
    }
}

impl<T> Update for ClientSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_maybe_update<U>(&self, _fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        None
    }
}

impl<T> Deref for ClientSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target = ArcRwSignal<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
