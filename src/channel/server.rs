use std::any::Any;
use std::sync::{Arc, RwLock};

use super::ChannelContext;
use crate::error::Error;
use crate::messages::{ChannelMessage, Messages};
use crate::traits::{ChannelHandler, ChannelSignalTrait, SendMapperHandler, private};
use crate::ws_signals::{ConnEntry, WsSignals};
use async_trait::async_trait;
use dashmap::DashMap;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::{Sender as BcSender, channel};

/// A signal owned by the server which writes to the websocket when mutated.
pub struct ServerChannelSignal<T, S = ()>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    name: String,
    observers: Arc<BcSender<(Option<String>, Messages)>>,
    server_callback: Arc<RwLock<Option<Arc<dyn ChannelHandler<T, S>>>>>,
    send_mapper: Arc<RwLock<Option<Arc<dyn SendMapperHandler<T, S>>>>>,
    connections: Arc<DashMap<String, Arc<DashMap<String, ConnEntry>>>>,
    state_factory: Arc<RwLock<Option<Arc<dyn Fn() -> S + Send + Sync>>>>,
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
            send_mapper: self.send_mapper.clone(),
            connections: self.connections.clone(),
            state_factory: self.state_factory.clone(),
        }
    }
}

#[async_trait]
impl<
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
    S: Send + Sync + Default + 'static,
> ChannelSignalTrait for ServerChannelSignal<T, S>
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
        state: &mut dyn Any,
        message: Value,
    ) -> Result<(), Error> {
        if let Ok(lock) = self.server_callback.read()
            && let Some(callback) = lock.as_ref()
            && let Ok(message) = serde_json::from_value(message)
        {
            let state: &mut S = state
                .downcast_mut()
                .expect("Connection state type mismatch");
            let mut ctx = ChannelContext::new(client_id.to_owned(), state);
            callback.handle(&mut ctx, &message);
        }

        Ok(())
    }

    fn create_state(&self) -> Box<dyn Any + Send + Sync> {
        if let Ok(lock) = self.state_factory.read()
            && let Some(factory) = lock.as_ref()
        {
            Box::new(factory())
        } else {
            Box::new(S::default())
        }
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

    /// Create or retrieve a channel signal, using an explicit `WsSignals` instance.
    ///
    /// Useful when running outside a Leptos server function context (e.g. in an Actix
    /// or Axum handler where you hold a reference to `WsSignals` from app state).
    pub fn new_with_context(signals: &mut WsSignals, name: &str) -> Result<Self, Error>
    where
        S: Default,
    {
        if let Some(signal) = signals.get_channel::<Self>(name) {
            return Ok(signal);
        }
        let (send, _) = channel(256);
        let new_signal = Self {
            name: name.to_owned(),
            observers: Arc::new(send),
            server_callback: Arc::new(RwLock::new(None)),
            send_mapper: Arc::new(RwLock::new(None)),
            connections: signals.channel_connections.clone(),
            state_factory: Arc::new(RwLock::new(None)),
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

    /// Register a handler called when a message arrives from a client.
    pub fn on_server(&self, callback: impl ChannelHandler<T, S>) -> Result<(), Error> {
        let Ok(mut server_callback) = self.server_callback.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *server_callback = Some(Arc::new(callback));
        Ok(())
    }

    /// Register a handler called when a message arrives from the server (no-op on server side).
    pub fn on_client(&self, _callback: impl ChannelHandler<T, S>) {}

    /// Register a per-connection send mapper.
    ///
    /// The mapper is called for each connected client before a message is sent. Return
    /// `Some(mapped_msg)` to deliver the (optionally transformed) message, or `None`
    /// to suppress it for that client.
    pub fn add_send_mapper(&self, mapper: impl SendMapperHandler<T, S>) -> Result<(), Error> {
        let Ok(mut lock) = self.send_mapper.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *lock = Some(Arc::new(mapper));
        Ok(())
    }

    /// Override the default per-connection state factory.
    ///
    /// By default, `S::default()` is used for each new connection. Use this to
    /// provide custom initial state. Must be called before clients connect.
    pub fn with_state_factory<F>(&self, factory: F) -> Result<(), Error>
    where
        F: Fn() -> S + Send + Sync + 'static,
    {
        let Ok(mut lock) = self.state_factory.write() else {
            return Err(Error::AddingChannelHandlerFailed);
        };
        *lock = Some(Arc::new(factory));
        Ok(())
    }

    /// Send a message to all connected clients.
    ///
    /// If a send mapper is registered, it is applied per-connection — the message
    /// is only delivered to clients where the mapper returns `Some(...)`. Without a
    /// mapper, the message is broadcast to all subscribed clients.
    pub fn send_message(&self, message: T) -> Result<(), Error> {
        if let Ok(lock) = self.send_mapper.read()
            && let Some(mapper) = lock.as_ref()
        {
            let map = self.connections.get(&self.name).map(|r| r.value().clone());
            let Some(map) = map else {
                return Ok(());
            };
            let keys: Vec<String> = map.iter().map(|e| e.key().clone()).collect();

            for key in keys {
                if let Some(mut entry) = map.get_mut(&key) {
                    let conn_id = key.as_str();
                    let s: &mut S = entry
                        .state
                        .downcast_mut()
                        .expect("Connection state type mismatch");
                    let ctx = ChannelContext::new(conn_id.to_owned(), s);
                    if let Some(mapped) = mapper.handle(&ctx, &message) {
                        let value = serde_json::to_value(&mapped)?;
                        let msg =
                            Messages::Channel(ChannelMessage::Message(self.name.clone(), value));
                        if entry.sender.try_send(Ok(msg)).is_err() {
                            tracing::warn!(
                                connection_id = %conn_id,
                                channel = %self.name,
                                "dropping message: per-connection channel full"
                            );
                        }
                    }
                }
            }
            Ok(())
        } else {
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

    /// Remove this channel and disconnect all subscribers.
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
