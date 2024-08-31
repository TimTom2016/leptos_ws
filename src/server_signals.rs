use crate::{error::Error, messages::ServerSignalUpdate, server_signal::ServerSignalTrait};
use leptos::prelude::*;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast::Receiver, RwLock};

#[derive(Clone)]
pub struct ServerSignals {
    signals: Arc<RwLock<HashMap<String, Arc<Box<dyn ServerSignalTrait + Send + Sync>>>>>,
}

impl ServerSignals {
    pub fn new() -> Self {
        let signals = Arc::new(RwLock::new(HashMap::new()));
        let me = Self { signals };
        me
    }

    pub async fn create_signal<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: String,
        value: T,
    ) -> Result<(), Error>
    where
        T: ServerSignalTrait,
    {
        if self
            .signals
            .write()
            .await
            .insert(name, Arc::new(Box::new(value)))
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
            .is_none()
        {
            Ok(())
        } else {
            Err(Error::AddingSignalFailed)
        }
    }
    pub async fn get_signal<T: Clone + 'static>(&mut self, name: String) -> Option<T> {
        self.signals
            .write()
            .await
            .get_mut(&name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }
    pub async fn add_observer(&self, name: String) -> Option<Receiver<ServerSignalUpdate>> {
        match self
            .signals
            .read()
            .await
            .get(&name)
            .map(|value| value.add_observer())
        {
            Some(fut) => Some(fut.await),
            None => None,
        }
    }

    pub async fn json(&self, name: String) -> Option<Result<Value, Error>> {
        match self
            .signals
            .read()
            .await
            .get(&name)
            .map(|value| value.json())
        {
            Some(res) => Some(res),
            None => None,
        }
    }
    pub async fn update(
        &self,
        name: String,
        patch: ServerSignalUpdate,
    ) -> Option<Result<(), Error>> {
        match self
            .signals
            .write()
            .await
            .get_mut(&name)
            .map(|value| value.update_json(patch))
        {
            Some(fut) => Some(fut.await),
            None => None,
        }
    }

    pub async fn contains(&self, name: &str) -> bool {
        self.signals.read().await.contains_key(name)
    }
}
