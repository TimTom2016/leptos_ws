use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::panic::Location;
use std::sync::Arc;

use crate::error::Error;
use crate::messages::ServerSignalUpdate;
use crate::server_signals::ServerSignals;
use async_trait::async_trait;
use futures::executor::block_on;
use guards::{Plain, ReadGuard};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::RwLock;

/// A signal owned by the server which writes to the websocket when mutated.
#[derive(Clone, Debug)]
pub struct ServerSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de>,
{
    initial: T,
    name: String,
    value: ArcRwSignal<T>,
    json_value: Arc<RwLock<Value>>,
    observers: Arc<Sender<ServerSignalUpdate>>,
}
#[async_trait]
pub trait ServerSignalTrait {
    async fn add_observer(&self) -> Receiver<ServerSignalUpdate>;
    async fn update_json(&self, patch: ServerSignalUpdate) -> Result<(), Error>;
    async fn update_if_changed(&self) -> Result<(), Error>;
    fn json(&self) -> Result<Value, Error>;
    fn as_any(&self) -> &dyn Any;
    fn track(&self);
}

#[async_trait]
impl<T> ServerSignalTrait for ServerSignal<T>
where
    T: Clone + Send + Sync + for<'de> Deserialize<'de> + 'static + Serialize,
{
    async fn add_observer(&self) -> Receiver<ServerSignalUpdate> {
        self.subscribe()
    }

    async fn update_json(&self, patch: ServerSignalUpdate) -> Result<(), Error> {
        let mut writer = self.json_value.write().await;
        if json_patch::patch(writer.deref_mut(), &patch.patch).is_ok() {
            //*self.value.write() = serde_json::from_value(writer.clone())?;
            let _ = self.observers.send(patch);
            Ok(())
        } else {
            Err(Error::UpdateSignalFailed)
        }
    }

    async fn update_if_changed(&self) -> Result<(), Error> {
        let json = self.json_value.read().await.clone();
        let new_json = serde_json::to_value(self.value.get())?;
        let mut res = Err(Error::UpdateSignalFailed);
        if json != new_json {
            res = self
                .update_json(ServerSignalUpdate::new_from_json(
                    self.name.clone(),
                    &json,
                    &new_json,
                ))
                .await;
        }
        res
    }

    fn json(&self) -> Result<Value, Error> {
        Ok(serde_json::to_value(self.value.get())?)
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
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(name: String, value: T) -> Result<Self, Error> {
        let mut signals = use_context::<ServerSignals>().ok_or(Error::MissingServerSignals)?;
        if block_on(signals.contains(&name)) {
            return Ok(block_on(signals.get_signal::<ServerSignal<T>>(name)).unwrap());
        }
        let (send, _) = channel(32);
        let new_signal = ServerSignal {
            initial: value.clone(),
            name: name.clone(),
            value: ArcRwSignal::new(value.clone()),
            json_value: Arc::new(RwLock::new(serde_json::to_value(value)?)),
            observers: Arc::new(send),
        };
        let signal = new_signal.clone();
        block_on(signals.create_signal(name, new_signal)).unwrap();
        Ok(signal)
    }

    pub fn subscribe(&self) -> Receiver<ServerSignalUpdate> {
        self.observers.subscribe()
    }
    fn check_is_hydrating(&self) -> bool {
        #[cfg(not(feature = "ssr"))]
        return false;
        let owner = match Owner::current() {
            Some(owner) => owner,
            None => return false,
        };
        let shared_context = match owner.shared_context() {
            Some(shared_context) => shared_context,
            None => return false,
        };
        #[cfg(feature = "ssr")]
        if shared_context.get_is_hydrating() || shared_context.during_hydration() == false {
            return true;
        }
        false
    }
}

impl<T> Update for ServerSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U> {
        let mut lock = self.value.try_write()?;
        let (did_update, val) = fun(&mut *lock);
        if !did_update {
            lock.untrack();
        } else {
        }
        drop(lock);
        block_on(async move {
            let _ = self.update_if_changed().await;
        });
        Some(val)
    }
}

impl<T> DefinedAt for ServerSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.value.defined_at()
    }
}

impl<T> ReadUntracked for ServerSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = ReadGuard<T, Plain<T>>;

    fn try_read_untracked(&self) -> Option<Self::Value> {
        if self.check_is_hydrating() {
            let guard: ReadGuard<T, Plain<T>> = ReadGuard::new(
                Plain::try_new(Arc::new(std::sync::RwLock::new(self.initial.clone()))).unwrap(),
            );
            return Some(guard);
        }

        self.value.try_read_untracked()
    }
}

impl<T> Get for ServerSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Value = T;

    fn try_get(&self) -> Option<Self::Value> {
        #[cfg(feature = "ssr")]
        if self.check_is_hydrating() {
            return Some(self.initial.clone());
        }
        self.value.try_get()
    }
}

impl<T> Deref for ServerSignal<T>
where
    T: Clone + Serialize + Send + Sync + for<'de> Deserialize<'de> + 'static,
{
    type Target = ArcRwSignal<T>;

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
