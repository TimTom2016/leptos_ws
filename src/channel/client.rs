use super::ChannelContext;
use crate::messages::{ChannelMessage, Messages};
use crate::traits::{ChannelHandler, ChannelSignalTrait, SendMapperHandler, private};
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast::{Sender, channel};

pub struct ClientChannelSignal<T, S = ()>
where
    T: Clone + Send + Sync,
{
    name: String,
    observers: Arc<Sender<(Option<String>, Messages)>>,
    client_callback: Arc<RwLock<Option<Arc<dyn ChannelHandler<T, S>>>>>,
    state: Arc<Mutex<Option<S>>>,
}

impl<T, S> Clone for ClientChannelSignal<T, S>
where
    T: Clone + Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            observers: self.observers.clone(),
            client_callback: self.client_callback.clone(),
            state: self.state.clone(),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static, S: Send + Sync + 'static>
    ChannelSignalTrait for ClientChannelSignal<T, S>
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Ok(self.observers.subscribe())
    }

    fn handle_message(
        &self,
        client_id: &str,
        _state: &mut dyn Any,
        message: Value,
    ) -> Result<(), Error> {
        if let Ok(lock) = self.client_callback.read()
            && let Some(callback) = lock.as_ref()
            && let Ok(message) = serde_json::from_value(message)
        {
            let mut state_lock = self.state.lock().unwrap();
            if let Some(ref mut state) = *state_lock {
                let mut ctx = ChannelContext::new(client_id.to_owned(), state);
                callback.handle(&mut ctx, &message);
            }
        }

        Ok(())
    }

    fn create_state(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(())
    }

    fn on_reconnect_message(&self) -> Result<Messages, Error> {
        Ok(Messages::Channel(ChannelMessage::Establish(
            self.name.clone(),
        )))
    }
}

impl<T, S> ClientChannelSignal<T, S>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
    S: Send + Sync + 'static,
{
    pub fn new(name: &str) -> Result<Self, Error>
    where
        S: Default,
    {
        Self::new_with_state(name, S::default())
    }

    pub fn new_with_state(name: &str, initial_state: S) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if let Some(signal) = signals.get_channel::<Self>(name) {
            return Ok(signal);
        }
        let (send, _) = channel(32);

        let new_signal = Self {
            name: name.to_owned(),
            observers: Arc::new(send),
            client_callback: Arc::new(RwLock::new(None)),
            state: Arc::new(Mutex::new(Some(initial_state))),
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
    pub fn on_server(&self, _callback: impl ChannelHandler<T, S>) {}

    /// Add a send mapper (no-op on client)
    pub fn add_send_mapper<F>(&self, _mapper: impl SendMapperHandler<T, S>) -> Result<(), Error> {
        Ok(())
    }

    /// Register a callback that gets called when a message arrives on the client side
    pub fn on_client(&self, callback: impl ChannelHandler<T, S>) -> Result<(), Error> {
        let Ok(mut client_callback) = self.client_callback.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *client_callback = Some(Arc::new(callback));
        Ok(())
    }

    /// Send a message to the server
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
}

impl<T, S> private::DeleteTrait for ClientChannelSignal<T, S>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
    S: Send + Sync + 'static,
{
    fn delete(&self) -> Result<(), Error> {
        #[cfg(any(feature = "csr", feature = "hydrate"))]
        if let Some(ws) = use_context::<crate::ServerSignalWebSocket>() {
            ws.send(&Messages::Channel(ChannelMessage::Delete(
                self.name.clone(),
            )))?;
        }
        Ok(())
    }
}
