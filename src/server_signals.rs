use crate::{error::Error, messages::ServerSignalUpdate, server_signal::ServerSignalTrait};
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

#[derive(Clone)]
pub struct ServerSignals {
    signals: Arc<DashMap<String, Arc<Box<dyn ServerSignalTrait + Send + Sync>>>>,
}

impl ServerSignals {
    pub fn new() -> Self {
        let signals = Arc::new(DashMap::new());
        let me = Self { signals };
        me
    }

    pub async fn create_signal<T>(&mut self, name: &str, value: T) -> Result<(), Error>
    where
        T: ServerSignalTrait + Clone + Send + Sync + 'static,
    {
        if self
            .signals
            .insert(name.to_owned(), Arc::new(Box::new(value)))
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
            .is_none()
        {
            Ok(())
        } else {
            Err(Error::AddingSignalFailed)
        }
    }
    pub async fn get_signal<T: Clone + 'static>(&mut self, name: &str) -> Option<T> {
        self.signals
            .get_mut(name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }
    pub async fn add_observer(&self, name: String) -> Option<Receiver<ServerSignalUpdate>> {
        match self.signals.get(&name) {
            Some(value) => Some(value.add_observer().await),
            None => None,
        }
    }

    pub async fn json(&self, name: String) -> Option<Result<Value, Error>> {
        match self.signals.get(&name) {
            Some(value) => Some(value.json()),
            None => None,
        }
    }
    pub async fn update(
        &self,
        name: String,
        patch: ServerSignalUpdate,
    ) -> Option<Result<(), Error>> {
        match self.signals.get_mut(&name) {
            Some(value) => Some(value.update_json(patch).await),
            None => None,
        }
    }

    pub async fn contains(&self, name: &str) -> bool {
        self.signals.contains_key(name)
    }
}
