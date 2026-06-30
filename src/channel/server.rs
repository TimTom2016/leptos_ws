use std::any::Any;
use std::sync::{Arc, RwLock};

use super::ChannelContext;
use crate::error::Error;
use crate::messages::{ChannelMessage, Messages};
use crate::traits::{ChannelHandler, ChannelSignalTrait, private};
use crate::ws_signals::WsSignals;
use async_trait::async_trait;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::{Sender, channel};

/// A signal owned by the server which writes to the websocket when mutated.
pub struct ServerChannelSignal<T, S = ()>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    name: String,
    observers: Arc<Sender<(Option<String>, Messages)>>,
    server_callback: Arc<RwLock<Option<Arc<dyn ChannelHandler<T, S>>>>>,
}

impl<T, S> Clone for ServerChannelSignal<T, S>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            observers: self.observers.clone(),
            server_callback: self.server_callback.clone(),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static, S: Send + Sync + Default + 'static>
    ChannelSignalTrait for ServerChannelSignal<T, S>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Ok(self.observers.subscribe())
    }

    fn handle_message(&self, client_id: &str, state: &mut dyn Any, message: Value) -> Result<(), Error> {
        if let Ok(lock) = self.server_callback.read()
            && let Some(callback) = lock.as_ref()
            && let Ok(message) = serde_json::from_value(message)
        {
            let state: &mut S = state.downcast_mut().expect("Connection state type mismatch");
            let mut ctx = ChannelContext::new(client_id.to_owned(), state);
            callback.handle(&mut ctx, &message);
        }

        Ok(())
    }

    fn create_state(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(S::default())
    }

    fn on_reconnect_message(&self) -> Result<Messages, Error> {
        Ok(Messages::Channel(ChannelMessage::Establish(
            self.name.clone(),
        )))
    }
}

impl<T, S> ServerChannelSignal<T, S>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
    S: Send + Sync + 'static,
{
    pub fn new(name: &str) -> Result<Self, Error>
    where
        S: Default,
    {
        let mut signals = use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        Self::new_with_context(&mut signals, name)
    }

    pub fn new_with_context(signals: &mut WsSignals, name: &str) -> Result<Self, Error>
    where
        S: Default,
    {
        if let Some(signal) = signals.get_channel::<Self>(name) {
            return Ok(signal);
        }
        let (send, _) = channel(32);
        let new_signal = Self {
            name: name.to_owned(),
            observers: Arc::new(send),
            server_callback: Arc::new(RwLock::new(None)),
        };
        let signal = new_signal.clone();

        match signals.create_channel(
            name,
            new_signal,
            &Messages::Channel(ChannelMessage::Establish(name.to_owned())),
        ) {
            Ok(()) => Ok(signal),
            Err(Error::AddingSignalFailed) => {
                signals.get_channel(name).ok_or(Error::AddingSignalFailed)
            }
            Err(e) => Err(e),
        }
    }

    /// Register a callback that gets called when a message arrives on the server side
    pub fn on_server(&self, callback: impl ChannelHandler<T, S>) -> Result<(), Error>
    {
        let Ok(mut server_callback) = self.server_callback.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *server_callback = Some(Arc::new(callback));
        Ok(())
    }

    /// Register a callback that gets called when a message arrives on the client side
    pub fn on_client(&self, _callback: impl ChannelHandler<T, S>)
    {
    }

    /// Send a message to the client
    pub fn send_message(&self, message: T) -> Result<(), Error> {
        let message = serde_json::to_value(&message)?;
        self.observers
            .send((
                None,
                Messages::Channel(ChannelMessage::Message(self.name.clone(), message)),
            ))
            .map_err(|_| Error::SendMessageFailed)?;

        Ok(())
    }

    pub fn delete(&self) -> Result<(), Error> {
        let mut signals = use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        signals.delete_channel(&self.name)
    }
}

impl<T, S> private::DeleteTrait for ServerChannelSignal<T, S>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    S: Send + Sync + 'static,
{
    fn delete(&self) -> Result<(), Error> {
        self.observers
            .send((
                None,
                Messages::Channel(ChannelMessage::Delete(self.name.clone())),
            ))
            .map_err(|_| Error::SendMessageFailed)?;
        Ok(())
    }
}
