use std::any::Any;
use std::sync::Arc;

use crate::error::Error;
use crate::messages::Messages;
use crate::messages::SignalUpdate;
use crate::traits::ChannelSignalTrait;
use crate::traits::WsSignalCore;
use dashmap::DashMap;
#[cfg(any(feature = "csr", feature = "hydrate", feature = "ssr"))]
use dashmap::mapref::entry::Entry;
use futures::channel::mpsc::Sender;
use leptos::server_fn::ServerFnError;
use serde_json::Value;
use tokio::sync::broadcast::Receiver;

pub(crate) struct ConnEntry {
    pub(crate) state: Box<dyn Any + Send + Sync>,
    pub(crate) sender: Sender<Result<Messages, ServerFnError>>,
}

#[derive(Clone)]
pub struct WsSignals {
    signals: Arc<DashMap<String, Arc<dyn WsSignalCore + Send + Sync + 'static>>>,
    channels: Arc<DashMap<String, Arc<dyn ChannelSignalTrait + Send + Sync + 'static>>>,
    pub(crate) channel_connections: Arc<DashMap<String, Arc<DashMap<String, ConnEntry>>>>,
}

impl WsSignals {
    pub fn new() -> Self {
        let signals = Arc::new(DashMap::new());
        let channels = Arc::new(DashMap::new());
        let channel_connections = Arc::new(DashMap::new());
        Self {
            signals,
            channels,
            channel_connections,
        }
    }

    pub fn create_signal<T>(&mut self, _name: &str, _value: T, _msg: &Messages) -> Result<(), Error>
    where
        T: WsSignalCore + Send + Sync + Clone + 'static,
    {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            use crate::ServerSignalWebSocket;

            let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;

            match self.signals.entry(_name.to_owned()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::new(_value));
                    ws.send(_msg)?;
                    Ok(())
                }
                Entry::Occupied(_) => Err(Error::AddingSignalFailed),
            }
        }

        #[cfg(all(feature = "ssr", not(any(feature = "hydrate", feature = "csr"))))]
        {
            match self.signals.entry(_name.to_owned()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::new(_value));
                    Ok(())
                }
                Entry::Occupied(_) => Err(Error::AddingSignalFailed),
            }
        }
        #[cfg(not(any(feature = "ssr", feature = "hydrate", feature = "csr")))]
        return Err(Error::AddingSignalFailed);
    }

    pub fn create_channel<T>(
        &mut self,
        _name: &str,
        _value: T,
        _msg: &Messages,
    ) -> Result<(), Error>
    where
        T: ChannelSignalTrait + Send + Sync + Clone + 'static,
    {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        {
            use crate::ServerSignalWebSocket;

            let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;

            match self.channels.entry(_name.to_owned()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::new(_value));
                    ws.send(_msg)?;
                    Ok(())
                }
                Entry::Occupied(_) => Err(Error::AddingSignalFailed),
            }
        }

        #[cfg(all(feature = "ssr", not(any(feature = "hydrate", feature = "csr"))))]
        {
            match self.channels.entry(_name.to_owned()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::new(_value));
                    Ok(())
                }
                Entry::Occupied(_) => Err(Error::AddingSignalFailed),
            }
        }
        #[cfg(not(any(feature = "ssr", feature = "hydrate", feature = "csr")))]
        return Err(Error::AddingSignalFailed);
    }

    /// Retrieve a registered signal by name, downcast to the requested type.
    pub fn get_signal<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.signals
            .get_mut(name)
            .and_then(|value| value.as_any().downcast_ref::<T>().cloned())
    }

    /// Retrieve a registered channel by name, downcast to the requested type.
    pub fn get_channel<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.channels
            .get_mut(name)
            .and_then(|value| value.as_any().downcast_ref::<T>().cloned())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }

    pub fn add_observer(&self, name: &str) -> Option<Receiver<(Option<String>, Messages)>> {
        self.signals
            .get(name)
            .and_then(|v| v.value().subscribe().ok())
    }

    pub fn create_channel_state(&self, name: &str) -> Option<Box<dyn Any + Send + Sync>> {
        self.channels.get(name).map(|v| v.create_state())
    }

    pub fn add_observer_channel(&self, name: &str) -> Option<Receiver<(Option<String>, Messages)>> {
        self.channels
            .get(name)
            .and_then(|v| v.value().subscribe().ok())
    }

    pub fn handle_message(
        &self,
        name: &str,
        client_id: &str,
        state: &mut dyn Any,
        message: Value,
    ) -> Option<Result<(), Error>> {
        self.channels
            .get(name)
            .map(|v| v.handle_message(client_id, state, message))
    }

    pub fn json(&self, name: &str) -> Option<Result<Value, Error>> {
        self.signals.get(name).map(|v| v.json())
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
            signal.1.delete()?;
            return Ok(());
        }
        Err(Error::DeletingSignalFailed)
    }

    pub fn delete_channel(&mut self, name: &str) -> Result<(), Error> {
        if let Some(signal) = self.channels.remove(name) {
            let _ = signal.1.delete();
            return Ok(());
        }
        Err(Error::DeletingChannelHandlerFailed)
    }

    pub fn get_reconnect_messages(&self) -> Vec<Messages> {
        let mut messages = Vec::new();
        for data in self.signals.iter() {
            if let Ok(message) = data.on_reconnect_message() {
                messages.push(message);
            }
        }

        for data in self.channels.iter() {
            if let Ok(message) = data.on_reconnect_message() {
                messages.push(message);
            }
        }
        messages
    }
}
