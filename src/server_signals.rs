use std::{any::{Any, TypeId}, borrow::{Borrow, Cow}, cell::RefCell, collections::HashMap, ops::{Deref, DerefMut}, rc::Rc, sync::Arc};

use leptos::{Effect, RwSignal, SignalWith};
use tokio::sync::RwLock;

#[cfg(all(feature="axum" ))]
use crate::axum::ServerSignal;
use crate::error::Error;


#[derive(Clone, Debug)]
pub struct ServerSignals {
    // References to these are kept by the closure for the callback
    // onmessage callback on the websocket
    signals: Arc<RwLock<HashMap<TypeId,Box<dyn Any>>>>,
}

impl ServerSignals {
    pub fn new() -> Self {
        Self {
            signals: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    pub async fn create_signal<T: Clone + 'static>(&mut self,value: T) -> Result<(),Error>
    {
        if self.signals.write().await.insert(TypeId::of::<T>(),Box::new(value)).map(|value| value.downcast_ref::<T>().unwrap().clone()).is_none() {
            Ok(())
        } else {
            Err(Error::AddingSignalFailed)
        }
    }
    pub async fn get_signal<T: Clone + 'static>(&mut self) -> Option<T> {
        self.signals.write().await.get_mut(&TypeId::of::<T>()).map(|value| {
            value.downcast_ref::<T>().unwrap().clone()
        })
    }
}