use crate::messages::{ChannelMessage, Messages};
use crate::traits::{ChannelSignalTrait, private};
use crate::{error::Error, ws_signals::WsSignals};
use async_trait::async_trait;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::Any;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast::{Sender, channel};

#[derive(Clone)]
pub struct ClientChannelSignal<T>
where
    T: Clone + Send + Sync,
{
    name: String,
    observers: Arc<Sender<(Option<String>, Messages)>>,
    client_callback: Arc<RwLock<Option<Arc<dyn Fn(&T) + Send + Sync + 'static>>>>,
}

#[async_trait]
impl<T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static> ChannelSignalTrait
    for ClientChannelSignal<T>
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
        if let Ok(lock) = self.client_callback.read()
            && let Some(callback) = lock.as_ref()
            && let Ok(message) = serde_json::from_value(message)
        {
            callback(&message);
        }

        Ok(())
    }

    fn on_reconnect_message(&self) -> Result<Messages, Error> {
        Ok(Messages::Channel(ChannelMessage::Establish(
            self.name.clone(),
        )))
    }
}

impl<T> ClientChannelSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if let Some(signal) = signals.get_channel(name) {
            return Ok(signal);
        }
        let (send, _) = channel(32);

        let new_signal = Self {
            name: name.to_owned(),
            observers: Arc::new(send),
            client_callback: Arc::new(RwLock::new(None)),
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
    pub fn on_server<F>(&self, _callback: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
    }

    /// Register a callback that gets called when a message arrives on the client side
    pub fn on_client<F>(&self, callback: F) -> Result<(), Error>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
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

impl<T> private::DeleteTrait for ClientChannelSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
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
