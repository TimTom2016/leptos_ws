use std::sync::Arc;

use crate::client_signal::ClientSignalTrait;
use crate::messages::Messages;
use crate::ServerSignalMessage;
use crate::ServerSignalWebSocket;
use crate::{error::Error, messages::ServerSignalUpdate};
use dashmap::DashMap;
use leptos::prelude::*;
use serde_json::Value;

#[derive(Clone)]
pub struct ClientSignals {
    signals: Arc<DashMap<String, Arc<Box<dyn ClientSignalTrait + Send + Sync>>>>,
}

impl ClientSignals {
    pub fn new() -> Self {
        let signals = Arc::new(DashMap::new());
        let me = Self { signals };
        me
    }

    pub fn create_signal<T>(&mut self, name: &str, value: T) -> Result<(), Error>
    where
        T: ClientSignalTrait + Clone + Send + Sync + 'static,
    {
        let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;
        if self
            .signals
            .insert(name.to_owned(), Arc::new(Box::new(value)))
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
            .is_none()
        {
            // Wrap the Establish message in ServerSignalMessage and Messages
            ws.send(&Messages::ServerSignal(ServerSignalMessage::Establish(
                name.to_owned(),
            )))?;
            Ok(())
        } else {
            Err(Error::AddingSignalFailed)
        }
    }

    pub fn reconnect(&self) -> Result<(), Error> {
        let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;

        // Get all signal names from the signals HashMap
        let signal_names: Vec<String> = self.signals.iter().map(|v| v.key().clone()).collect();

        // Resend establish message for each signal
        for name in signal_names {
            ws.send(&Messages::ServerSignal(ServerSignalMessage::Establish(
                name,
            )))?;
        }

        Ok(())
    }

    pub fn get_signal<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.signals
            .get_mut(name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn update(&self, name: &str, patch: ServerSignalUpdate) -> Option<Result<(), Error>> {
        self.signals
            .get_mut(name)
            .map(|value| value.update_json(patch))
    }

    pub fn set_json(&self, name: &str, new_value: Value) -> Option<Result<(), Error>> {
        self.signals
            .get_mut(name)
            .map(|value| value.set_json(new_value))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }
}
