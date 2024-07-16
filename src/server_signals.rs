use std::{
    any::{Any, TypeId},
    borrow::{Borrow, Cow},
    cell::RefCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::Arc,
};
use crate::{error::Error, server_signal::ServerSignalTrait, messages::ServerSignalUpdate};
use futures::executor::block_on;
use json_patch::Patch;
use leptos::prelude::*;
use tokio::sync::{broadcast::Receiver, RwLock};

#[derive(Clone)]
pub struct ServerSignals {
    signals: Arc<RwLock<HashMap<String, Arc<Box<dyn ServerSignalTrait + Send + Sync>>>>>,
    count: ArcRwSignal<u16>,
}

impl ServerSignals {
    pub fn new() -> Self {
        let count = ArcRwSignal::new(0);
        let count2 = count.clone();
        let signals = Arc::new(RwLock::new(HashMap::new()));
        let me = Self {
            signals,
            count
        };
        me.setup_effect();
        me
    }

    pub fn setup_effect(&self) {
        let count = self.count.clone();
        let signals = self.signals.clone();
        Effect::new_isomorphic(move |_| {
            count.track();
            
            for signal in block_on(signals.read())
                .iter()
            {
                signal.1.track();
                // block_on(signal.1.update_if_changed());
            }
            println!("Hello World");
            for signal in block_on(signals.read())
                .iter()
            {
                // signal.1.update_if_changed();
                block_on(signal.1.update_if_changed());
            }


        });
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
            self.count.update(|value| *value += 1);
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
    pub async fn add_observer(&self,name: String) -> Option<Receiver<ServerSignalUpdate>> {
        match self.signals
            .read()
            .await
            .get(&name)
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
            .map(|value| value.update_json(patch)) {
                Some(fut) => Some(fut.await),
                None => None,
            }
    }
    pub async fn update_changed_function(&self) -> bool {
        let mut found = false;
        for signal in self.signals
            .read()
            .await
            .iter() {
            if signal.1.update_if_changed().await.is_ok() == true {
                found = true;
            }
        }
        found
    }
    #[track_caller]
    pub fn track_all(&self) {
        for signal in self.signals
            .blocking_read()
            .iter()
        {
            signal.1.track();
        }
    }
}
