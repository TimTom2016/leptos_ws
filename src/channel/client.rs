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
    // fn send_to_server(&self, data: T) -> Result<(), Error> {
    //     use crate::ServerSignalWebSocket;

    //     let ws = use_context::<ServerSignalWebSocket>().ok_or(Error::MissingServerSignals)?;
    //     let json_data =
    //         serde_json::to_value(&data).map_err(|err| Error::SerializationFailed(err))?;

    //     // Create a patch update message to send to server
    //     let current_json = self.json()?;
    //     let patch = json_patch::diff(&current_json, &json_data);
    //     let update = crate::messages::SignalUpdate::new_from_patch(self.name.clone(), &patch);

    //     ws.send(&Messages::ServerSignal(ServerSignalMessage::Update(update)))?;

    //     // Trigger server callbacks (simulate server-side processing)
    //     if let Ok(callbacks) = self.server_callbacks.read() {
    //         for callback in callbacks.iter() {
    //             callback(&data);
    //         }
    //     }

    //     Ok(())
    // }
    //
    //

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn subscribe(
        &self,
    ) -> Result<tokio::sync::broadcast::Receiver<(Option<String>, Messages)>, Error> {
        Ok(self.observers.subscribe())
    }

    fn handle_message(&self, message: Value) -> Result<(), Error> {
        if let Ok(lock) = self.client_callback.read() {
            if let Some(callback) = lock.as_ref() {
                if let Ok(message) = serde_json::from_value(message) {
                    callback(&message);
                }
            }
        }

        Ok(())
    }
}

impl<T> ClientChannelSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: &str) -> Result<Self, Error> {
        let mut signals: WsSignals =
            use_context::<WsSignals>().ok_or(Error::MissingServerSignals)?;
        if let Some(signal) = signals.get_channel::<ClientChannelSignal<T>>(name) {
            return Ok(signal);
        }
        let (send, _) = channel(32);

        let new_signal = Self {
            name: name.to_owned(),
            observers: Arc::new(send),
            client_callback: Arc::new(RwLock::new(None)),
        };
        let signal = new_signal.clone();
        signals.create_channel(
            name,
            new_signal,
            &Messages::Channel(ChannelMessage::Establish(name.to_owned())),
        )?;
        Ok(signal)
    }

    /// Register a callback that gets called when a message arrives on the server side
    pub fn on_server<F>(&self, _callback: F) -> Result<(), Error>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        Ok(())
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
        self.observers.send((
            None,
            Messages::Channel(ChannelMessage::Message(self.name.clone(), message)),
        ));

        Ok(())
    }
}

impl<T> private::DeleteTrait for ClientChannelSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn delete(&self) -> Result<(), Error> {
        Err(Error::NotAvailableOnClient)
    }
}
