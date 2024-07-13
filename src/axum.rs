use std::any::type_name;
use std::marker::PhantomData;
use std::sync::Arc;
use std::{any::TypeId, borrow::Cow};
use std::ops::{self, Deref};

use axum::extract::ws::Message;
use futures::sink::{Sink, SinkExt};
use json_patch::Patch;
use leptos::{expect_context, use_context, RwSignal, SignalGet, SignalSet, SignalUpdate, SignalWith};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use crate::server_signals::ServerSignals;
use crate::ServerSignalUpdate;
use crate::error::Error;

/// A signal owned by the server which writes to the websocket when mutated.
#[derive(Clone, Debug)]
pub struct ServerSignal<T>
where 
    T: 'static + Clone + SignalGet + SignalSet + SignalWith + SignalUpdate
{
    value: T,
    json_value: Value,
    observers: Arc<Sender::<Patch>>
}

impl<T> ServerSignal<T>
where 
    T: 'static + Clone + SignalGet + SignalSet + SignalWith + SignalUpdate,
    <T as SignalGet>::Value: Serialize
{
    pub async fn new(value: T) -> Result<Self,Error> {
        let mut signals = use_context::<ServerSignals>().ok_or(Error::MissingServerSignals)?;
        let (send,recv) = channel(32);
        let new_signal = ServerSignal {
            value: value.clone(),
            json_value: serde_json::to_value(value.get())?,
            observers: Arc::new(send),
        };
        let signal = new_signal.clone();
        signals.create_signal(new_signal).await?;
        Ok(signal)
    }

    pub async fn subscribe(&self) -> Receiver<Patch> {
        self.observers.subscribe()
    }
}


impl<T> Deref for ServerSignal<T>
where
    T: 'static + Clone + SignalGet + SignalSet + SignalWith + SignalUpdate,
{
    type Target=T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}









// impl<T> SignalGet for ServerSignal<T>
// where 
//     T: 'static + Clone + SignalGet+ SignalSet + SignalWith + SignalUpdate,
// {
//     type Value = <T as SignalGet>::Value;
//     #[inline(always)]
//     fn get(&self) -> Self::Value {
//         self.value.get()
//     }
//     #[inline(always)]
//     fn try_get(&self) -> Option<Self::Value> {
//         self.value.try_get()
//     }
// }

// impl<T> SignalSet for ServerSignal<T>
// where 
//     T: 'static + Clone + SignalGet+ SignalSet + SignalWith + SignalUpdate,

// {
//     type Value = <T as SignalSet>::Value;

//     #[inline(always)]
//     fn set(&self, new_value: Self::Value) {
//         self.value.set(new_value)
//     }
//     #[inline(always)]
//     fn try_set(&self, new_value: Self::Value) -> Option<Self::Value> {
//         self.value.try_set(new_value)
//     }
// }

// impl<T> SignalUpdate for ServerSignal<T>
// where
//     T: 'static + Clone + SignalGet+ SignalSet + SignalWith + SignalUpdate,
// {
//     type Value = <T as SignalUpdate>::Value;

//     #[inline(always)]
//     fn update(&self, f: impl FnOnce(&mut Self::Value)) {
//         self.value.update(f)
//     }

//     #[inline(always)]
//     fn try_update<O>(&self, f: impl FnOnce(&mut Self::Value) -> O)
//         -> Option<O> {
//         self.value.try_update(f)
//     }
// }

// impl<T> SignalWith for ServerSignal<T>
// where
//     T: 'static + Clone + SignalGet+ SignalSet + SignalWith + SignalUpdate,
// {
//     type Value=<T as SignalWith>::Value;
//     #[inline(always)]
//     fn with<O>(&self, f: impl FnOnce(&Self::Value) -> O) -> O {
//         self.value.with(f)
//     }
//     #[inline(always)]
//     fn try_with<O>(&self, f: impl FnOnce(&Self::Value) -> O) -> Option<O> {
//         self.value.try_with(f)
//     }
// }


























// impl<T> ServerSignal<T>
// {
//     /// Creates a new [`ServerSignal`], initializing `T` to default.
//     ///
//     /// This function can fail if serilization of `T` fails.
//     pub fn new() -> Result<Self, serde_json::Error>
//     where
//         T: Default + Serialize,
//     {
//         let signals = expect_context::<ServerSignals>();
//         Ok(ServerSignal {
//             value: T::default(),
//             json_value: serde_json::to_value(T::default())?,
//         })
//     }

//     /// Modifies the signal in a closure, and sends the json diffs through the websocket connection after modifying.
//     ///
//     /// The same websocket connection should be used for a given client, otherwise the signal could become out of sync.
//     ///
//     /// # Example
//     ///
//     /// ```ignore
//     /// let count = ServerSignal::new("counter").unwrap();
//     /// count.with(&mut websocket, |count| {
//     ///     count.value += 1;
//     /// }).await?;
//     /// ```
//     pub async fn with<'e, O, S>(
//         &'e mut self,
//         sink: &mut S,
//         f: impl FnOnce(&mut T) -> O,
//     ) -> Result<O, Error>
//     where
//         T: Clone + Serialize + 'static,
//         S: Sink<Message> + Unpin,
//         axum::Error: From<<S as Sink<Message>>::Error>,
//     {
//         let output = f(&mut self.value);
//         let new_json = serde_json::to_value(self.value.clone())?;
//         let update =
//             ServerSignalUpdate::new_from_json::<T>(type_name::<T>(), &self.json_value, &new_json);
//         let update_json = serde_json::to_string(&update)?;
//         sink.send(Message::Text(update_json))
//             .await
//             .map_err(|err| Error::WebSocket(err.into()))?;
//         self.json_value = new_json;
//         Ok(output)
//     }

//     /// Consumes the [`ServerSignal`], returning the inner value.
//     pub fn into_value(self) -> T {
//         self.value
//     }

//     /// Consumes the [`ServerSignal`], returning the inner json value.
//     pub fn into_json_value(self) -> Value {
//         self.json_value
//     }
// }

// impl<T> ops::Deref for ServerSignal<T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         &self.value
//     }
// }

// impl<T> AsRef<T> for ServerSignal<T> {
//     fn as_ref(&self) -> &T {
//         &self.value
//     }
// }

// /// A server signal error.
// #[derive(Debug, Error)]
// pub enum Error {
//     /// Serialization of the signal value failed.
//     #[error(transparent)]
//     SerializationFailed(#[from] serde_json::Error),
//     /// The websocket returned an error.
//     #[error(transparent)]
//     WebSocket(#[from] axum::Error),
// }