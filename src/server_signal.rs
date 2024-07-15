use std::any::{type_name, Any};
use std::marker::PhantomData;
use std::ops::{self, Deref, DerefMut};
use std::panic::Location;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{any::TypeId, borrow::Cow};

use crate::error::Error;
use crate::server_signals::ServerSignals;
use crate::ServerSignalUpdate;
use axum::async_trait;
use futures::sink::{Sink, SinkExt};
use graph::ReactiveNode;
use guards::{UntrackedWriteGuard, WriteGuard};
use json_patch::Patch;
use leptos::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::spawn;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::RwLock;

/// A signal owned by the server which writes to the websocket when mutated.
#[derive(Clone, Debug)]
pub struct ServerSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de>,
{
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
    observers: Arc<Sender<ServerSignalUpdate>>,
}
#[async_trait]
pub trait ServerSignalTrait {
    async fn add_observer(&self) -> Receiver<ServerSignalUpdate>;
    async fn update(&self,patch: ServerSignalUpdate) -> Result<(),Error>;
    fn as_any(&self) -> &dyn Any;
    fn track(&self);
}

#[async_trait]
impl<T> ServerSignalTrait for ServerSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> +'static + Serialize,
{
    async fn add_observer(&self) -> Receiver<ServerSignalUpdate> {
        self.add_observer().await
    }

    async fn update(&self,patch: ServerSignalUpdate) -> Result<(),Error> {
        let mut writer = self.json_value.write().await;
        if json_patch::patch(writer.deref_mut(), &patch.patch).is_ok() {
            self.value.set(serde_json::from_value(writer.clone())?);
            self.observers.send(patch).unwrap();
            Ok(())
        } else {
            Err(Error::UpdateSignalFailed)
        }
    }


    fn as_any(&self) -> &dyn Any {
        self
    }

    #[track_caller]
    fn track(&self) {
        self.value.track()
    }
}




impl<T> ServerSignal<T>
where
    T:  Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub async fn new(name: String,value: impl Into<ArcRwSignal<T>>) -> Result<Self, Error> {
        let mut signals = use_context::<ServerSignals>().ok_or(Error::MissingServerSignals)?;
        let (send, recv) = channel(32);
        let rs_sig = value.into();
        let new_signal = ServerSignal {
            name: name.clone(),
            value: rs_sig.clone(),
            json_value: Arc::new(RwLock::new(serde_json::to_value(rs_sig.get())?)),
            observers: Arc::new(send),
        };
        let signal = new_signal.clone();
        signals.create_signal(name,new_signal).await?;
        Ok(signal)
    }

    pub async fn subscribe(&self) -> Receiver<ServerSignalUpdate> {
        self.observers.subscribe()
    }

    

}

impl<T> Deref for ServerSignal<T>
where
    T:  Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target=ArcRwSignal<T>;

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
