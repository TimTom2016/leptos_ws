use std::any::Any;
use std::sync::{Arc, RwLock};

use crate::error::Error;
use crate::messages::{ChannelMessage, Messages};
use crate::traits::ChannelSignalTrait;
use crate::ws_signals::WsSignals;
use async_trait::async_trait;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::{channel, Sender};

/// A signal owned by the server which writes to the websocket when mutated.
#[derive(Clone)]
pub struct ServerChannelSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de>,
{
    name: String,
    observers: Arc<Sender<(Option<String>, Messages)>>,
    server_callback: Arc<RwLock<Option<Arc<dyn Fn(&T) + Send + Sync + 'static>>>>,
}

#[async_trait]
impl<T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static> ChannelSignalTrait
    for ServerChannelSignal<T>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Ok(self.observers.subscribe())
    }

    fn handle_message(&self, message: Value) -> Result<(), Error> {
        if let Ok(lock) = self.server_callback.read() {
            if let Some(callback) = lock.as_ref() {
                if let Ok(message) = serde_json::from_value(message) {
                    callback(&message);
                }
            }
        }

        Ok(())
    }
}

impl<T> ServerChannelSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str) -> Result<Self, Error> {
        let mut signals = use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if let Some(signal) = signals.get_channel::<ServerChannelSignal<T>>(name) {
            return Ok(signal);
        }
        let (send, _) = channel(32);
        let new_signal = ServerChannelSignal {
            name: name.to_owned(),
            observers: Arc::new(send),
            server_callback: Arc::new(RwLock::new(None)),
        };
        let signal = new_signal.clone();
        signals.create_channel(
            name,
            new_signal,
            &Messages::Channel(ChannelMessage::Establish(name.to_owned())),
        )?;
        Ok(signal)
    }

    fn check_is_hydrating(&self) -> bool {
        #[cfg(feature = "ssr")]
        {
            let owner = match Owner::current() {
                Some(owner) => owner,
                None => return false,
            };
            let shared_context = match owner.shared_context() {
                Some(shared_context) => shared_context,
                None => return false,
            };
            return shared_context.get_is_hydrating() || !shared_context.during_hydration();
        }
        #[allow(unreachable_code)]
        false
    }

    /// Register a callback that gets called when a message arrives on the server side
    pub fn on_server<F>(&self, callback: F) -> Result<(), Error>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let Ok(mut server_callback) = self.server_callback.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *server_callback = Some(Arc::new(callback));
        Ok(())
    }

    /// Register a callback that gets called when a message arrives on the client side
    pub fn on_client<F>(&self, _callback: F) -> Result<(), Error>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        Ok(())
    }

    /// Send a message to the client
    pub fn send_message(&self, message: T) -> Result<(), Error> {
        let message = serde_json::to_value(&message)?;
        self.observers.send((
            None,
            Messages::Channel(ChannelMessage::Message(self.name.clone(), message)),
        ));

        Ok(())
    }
}
