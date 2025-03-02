use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::client_signal::ClientSignalTrait;
use crate::messages::Messages;
use crate::ServerSignalMessage;
use crate::ServerSignalWebSocket;
use crate::{error::Error, messages::ServerSignalUpdate};
use leptos::prelude::*;
use serde_json::Value;

#[derive(Clone)]
pub struct ClientSignals {
    signals: Arc<RwLock<HashMap<String, Arc<Box<dyn ClientSignalTrait + Send + Sync>>>>>,
}

impl ClientSignals {
    pub fn new() -> Self {
        let signals = Arc::new(RwLock::new(HashMap::new()));
        let me = Self { signals };
        me
    }

    pub fn create_signal<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: String,
        value: T,
    ) -> Result<(), Error>
    where
        T: ClientSignalTrait,
    {
        let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;
        if self
            .signals
            .write()
            .unwrap()
            .insert(name.clone(), Arc::new(Box::new(value)))
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
            .is_none()
        {
            // Wrap the Establish message in ServerSignalMessage and Messages
            ws.send(&Messages::ServerSignal(ServerSignalMessage::Establish(
                name.clone(),
            )))?;
            Ok(())
        } else {
            Err(Error::AddingSignalFailed)
        }
    }

    pub fn reconnect(&self) -> Result<(), Error> {
        let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;

        // Get all signal names from the signals HashMap
        let signal_names: Vec<String> = self.signals.read().unwrap().keys().cloned().collect();

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
            .write()
            .unwrap()
            .get_mut(name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }

    pub fn update(&self, name: &str, patch: ServerSignalUpdate) -> Option<Result<(), Error>> {
        match self
            .signals
            .write()
            .unwrap()
            .get_mut(name)
            .map(|value| value.update_json(patch))
        {
            Some(fut) => Some(fut),
            None => None,
        }
    }

    pub fn set_json(&self, name: &str, new_value: Value) -> Option<Result<(), Error>> {
        match self
            .signals
            .write()
            .unwrap()
            .get_mut(name)
            .map(|value| value.set_json(new_value))
        {
            Some(res) => Some(res),
            None => None,
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.signals.read().unwrap().contains_key(name)
    }
}
