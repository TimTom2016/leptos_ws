use std::{
    any::{Any, TypeId},
    borrow::{Borrow, Cow},
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
};
use crate::{error::Error, server_signal::ServerSignalTrait, ServerSignalUpdate};
use json_patch::Patch;
use leptos::prelude::*;
use tokio::sync::{broadcast::Receiver, RwLock};

#[derive(Clone)]
pub struct ServerSignals {
    // References to these are kept by the closure for the callback
    // onmessage callback on the websocket
    signals: Arc<RwLock<HashMap<String, Arc<Box<dyn ServerSignalTrait + Send + Sync>>>>>,
}

impl ServerSignals {
    pub fn new() -> Self {
        Self {
            signals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_signal<T: Clone + Send + Sync + 'static>(
        &mut self,
        name: String,
        value: T,
    ) -> Result<(), Error> 
    where 
        T: ServerSignalTrait
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
    pub async fn get_signal<T: Clone + 'static>(&mut self,name: String) -> Option<T> {
        self.signals
            .write()
            .await
            .get_mut(&name)
            .map(|value| value.as_any().downcast_ref::<T>().unwrap().clone())
    }
    pub async fn add_subscriber(&self,name: String) -> Option<Receiver<ServerSignalUpdate>> {
        match self.signals
            .write()
            .await
            .get_mut(&name)
            .map(|value| value.add_observer()) {
                Some(fut) => Some(fut.await),
                None => None,
            }
    }
    pub async fn update(&self,name: String,patch: ServerSignalUpdate) -> Option<Result<(),Error>> {
        match self.signals
            .write()
            .await
            .get_mut(&name)
            .map(|value| value.update(patch)) {
                Some(fut) => Some(fut.await),
                None => None,
            }
    }
}
