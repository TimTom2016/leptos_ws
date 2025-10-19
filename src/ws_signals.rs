use std::sync::Arc;

use crate::error::Error;
use crate::messages::Messages;
use crate::messages::SignalUpdate;
use crate::traits::ChannelSignalTrait;
use crate::traits::WsSignalCore;
use dashmap::DashMap;
use leptos::prelude::*;
use serde_json::Value;
use tokio::sync::broadcast::Receiver;

#[derive(Clone)]
pub struct WsSignals {
    signals: Arc<DashMap<String, Arc<dyn WsSignalCore + Send + Sync + 'static>>>,
    channels: Arc<DashMap<String, Arc<dyn ChannelSignalTrait + Send + Sync + 'static>>>,
}

impl WsSignals {
    pub fn new() -> Self {
        let signals = Arc::new(DashMap::new());
        let channels = Arc::new(DashMap::new());
        let me = Self { signals, channels };
        me
    }
    pub fn create_signal<T>(&mut self, name: &str, value: T, msg: &Messages) -> Result<(), Error>
    where
        T: WsSignalCore + Send + Sync + Clone + 'static,
    {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            use crate::ServerSignalWebSocket;

            let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;
            if self
                .signals
                .insert(name.to_owned(), Arc::new(value))
                .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
                .is_none()
            {
                // Wrap the Establish message in ServerSignalMessage and Messages
                ws.send(msg)?;
                return Ok(());
            }
        }

        #[cfg(all(feature = "ssr", not(any(feature = "hydrate", feature = "csr"))))]
        {
            if self
                .signals
                .insert(name.to_owned(), Arc::new(value))
                .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
                .is_none()
            {
                return Ok(());
            }
        }
        Err(Error::AddingSignalFailed)
    }

    pub fn create_channel<T>(&mut self, name: &str, value: T, msg: &Messages) -> Result<(), Error>
    where
        T: ChannelSignalTrait + Send + Sync + Clone + 'static,
    {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            use crate::ServerSignalWebSocket;

            let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;
            if self
                .channels
                .insert(name.to_owned(), Arc::new(value))
                .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
                .is_none()
            {
                // Wrap the Establish message in ServerSignalMessage and Messages
                ws.send(msg)?;
                return Ok(());
            }
        }

        #[cfg(all(feature = "ssr", not(any(feature = "hydrate", feature = "csr"))))]
        {
            if self
                .channels
                .insert(name.to_owned(), Arc::new(value))
                .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
                .is_none()
            {
                return Ok(());
            }
        }
        Err(Error::AddingSignalFailed)
    }

    pub fn get_signal<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.signals
            .get_mut(name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn get_channel<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.channels
            .get_mut(name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }

    pub fn add_observer(&self, name: &str) -> Option<Receiver<(Option<String>, Messages)>> {
        match self.signals.get(name) {
            Some(value) => value.value().subscribe().ok(),
            None => None,
        }
    }

    pub fn add_observer_channel(&self, name: &str) -> Option<Receiver<(Option<String>, Messages)>> {
        match self.channels.get(name) {
            Some(value) => value.value().subscribe().ok(),
            None => None,
        }
    }

    pub fn handle_message(&self, name: &str, message: Value) -> Option<Result<(), Error>> {
        match self.channels.get(name) {
            Some(value) => Some(value.handle_message(message)),
            None => None,
        }
    }

    pub fn json(&self, name: &str) -> Option<Result<Value, Error>> {
        match self.signals.get(name) {
            Some(value) => Some(value.json()),
            None => None,
        }
    }
    pub async fn update(
        &self,
        name: &str,
        patch: SignalUpdate,
        id: Option<String>,
    ) -> Option<Result<(), Error>> {
        match self.signals.get_mut(name) {
            Some(value) => Some(value.update_json(patch.get_patch(), id).await),
            None => None,
        }
    }

    pub fn set_json(&self, name: &str, new_value: Value) -> Option<Result<(), Error>> {
        self.signals
            .get_mut(name)
            .map(|value| value.set_json(new_value))
    }

    pub fn delete_signal(&mut self, name: &str) -> Result<(), Error> {
        if let Some(signal) = self.signals.remove(name) {
            signal.1.delete();
            return Ok(());
        }
        Err(Error::DeletingSignalFailed)
    }

    pub fn delete_channel(&mut self, name: &str) -> Result<(), Error> {
        if let Some(signal) = self.channels.remove(name) {
            signal.1.delete();
            return Ok(());
        }
        Err(Error::DeletingChannelHandlerFailed)
    }
}
