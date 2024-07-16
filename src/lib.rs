use json_patch::Patch;
use leptos::reactive_graph::signal::ArcRwSignal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
};
use wasm_bindgen::JsValue;
use web_sys::WebSocket;

pub mod messages;
pub mod error;
#[cfg(feature="ssr")]
pub mod server_signal;

#[cfg(feature = "ssr")]
pub mod server_signals;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSignalWebSocket {
    ws: WebSocket,
    // References to these are kept by the closure for the callback
    // onmessage callback on the websocket
    state_signals: Rc<RefCell<HashMap<Cow<'static, str>, RwSignal<serde_json::Value>>>>,
    // When the websocket is first established, the leptos may not have
    // completed the traversal that sets up all of the state signals.
    // Without that, we don't have a base state to apply the patches to,
    // and therefore we must keep a record of the patches to apply after
    // the state has been set up.
    delayed_updates: Rc<RefCell<HashMap<Cow<'static, str>, Vec<Patch>>>>,
}

#[cfg(target_arch = "wasm32")]
impl ServerSignalWebSocket {
    /// Returns the inner websocket.
    pub fn ws(&self) -> WebSocket {
        self.ws.clone()
    }
}

#[cfg(all(feature = "axum", feature = "ssr"))]
mod axum;

#[cfg(target_arch = "wasm32")]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<Option<WebSocket>, JsValue> {
    use js_sys::{Function, JsString};
    use leptos::{use_context, SignalUpdate};
    use wasm_bindgen::{prelude::Closure, JsCast};
    use web_sys::MessageEvent;

    if use_context::<ServerSignalWebSocket>().is_none() {
        let ws = WebSocket::new(url)?;
        provide_context(ServerSignalWebSocket {
            ws,
            state_signals: Rc::default(),
            delayed_updates: Rc::default(),
        });
    }

    let ws = use_context::<ServerSignalWebSocket>().unwrap();

    let handlers = ws.state_signals.clone();
    let delayed_updates = ws.delayed_updates.clone();

    let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        let ws_string = event
            .data()
            .dyn_into::<JsString>()
            .unwrap()
            .as_string()
            .unwrap();
        if let Ok(update_signal) = serde_json::from_str::<ServerSignalUpdate>(&ws_string) {
            let handler_map = (*handlers).borrow();
            let name = &update_signal.name;
            let mut delayed_map = (*delayed_updates).borrow_mut();
            if let Some(signal) = handler_map.get(name) {
                if let Some(delayed_patches) = delayed_map.remove(name) {
                    signal.update(|doc| {
                        for patch in delayed_patches {
                            json_patch::patch(doc, &patch).unwrap();
                        }
                    });
                }
                signal.update(|doc| {
                    json_patch::patch(doc, &update_signal.patch).unwrap();
                });
            } else {
                leptos::logging::warn!("No local state for update to {}. Queuing patch.", name);
                delayed_map
                    .entry(name.clone())
                    .or_default()
                    .push(update_signal.patch.clone());
            }
        }
    }) as Box<dyn FnMut(_)>);
    let function: &Function = callback.as_ref().unchecked_ref();
    ws.ws.set_onmessage(Some(function));

    // Keep the closure alive for the lifetime of the program
    callback.forget();

    Ok(Some(ws.ws()))
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
fn provide_websocket_inner(_url: &str) -> Result<Option<WebSocket>, JsValue> {
    use wasm_bindgen::JsValue;
    use web_sys::WebSocket;

    Ok(None)
}

/// A server signal update containing the signal type name and json patch.
///
/// This is whats sent over the websocket, and is used to patch the signal if the type name matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSignalUpdate {
    name: Cow<'static, str>,
    patch: Patch,
}

impl ServerSignalUpdate {
    /// Creates a new [`ServerSignalUpdate`] from an old and new instance of `T`.
    pub fn new<T>(
        name: impl Into<Cow<'static, str>>,
        old: &T,
        new: &T,
    ) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        let left = serde_json::to_value(old)?;
        let right = serde_json::to_value(new)?;
        let patch = json_patch::diff(&left, &right);
        Ok(ServerSignalUpdate {
            name: name.into(),
            patch,
        })
    }

    /// Creates a new [`ServerSignalUpdate`] from two json values.
    pub fn new_from_json(name: impl Into<Cow<'static, str>>, old: &Value, new: &Value) -> Self {
        let patch = json_patch::diff(old, new);
        ServerSignalUpdate {
            name: name.into(),
            patch,
        }
    }
}

#[derive(Clone, Serialize,Deserialize)]
pub struct History(pub Vec<f64>);

impl Into<ArcRwSignal<History>> for History {
    fn into(self) -> ArcRwSignal<History> {
        ArcRwSignal::new(self)
    }
}

#[cfg(test)]
mod test {
    use std::{time::Duration, vec};

    use any_spawner::Executor;
    use leptos::prelude::*;
    use tokio::{spawn, task, time::sleep};
    use web_sys::js_sys::Math::sign;

    use crate::{
        error, server_signal::{ServerSignal, ServerSignalTrait}, server_signals::{self, ServerSignals}, History
    };

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    pub async fn server_signal_test() {
        let history = "History".to_string();
        _ = Executor::init_tokio();
        let _owner = Owner::new();
        _owner.set();
        let server_signals = ServerSignals::new();
        provide_context(server_signals.clone());
        use_context::<ServerSignals>().unwrap();
        let signal = ServerSignal::new(history.clone(),History(vec![1.0, 2.0, 3.0, 4.0]))
            .unwrap();
        let text = RwSignal::new("".to_string());
        let signal2 = signal.clone();
        signal2.update(|values| values.0.push(5.0));
        signal2.update(|values| values.0.push(5.0 + 1 as f64));
        signal2.update(|values| values.0.push(5.0 + 2 as f64));
    }
    
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    pub async fn observer_test() {
        let history = "History".to_string();
        _ = Executor::init_tokio();
        let _owner = Owner::new();
        _owner.set();
        let server_signals = ServerSignals::new();
        provide_context(server_signals.clone());
        use_context::<ServerSignals>().unwrap();
        let signal = ServerSignal::new(history.clone(),History(vec![1.0, 2.0, 3.0, 4.0]))
            .unwrap();
        let mut observer = signal.add_observer().await;
        let signal2 = signal.clone();
        signal2.update(|values| values.0.push(5.0));
        Executor::tick().await;
        println!("{:?}",observer.recv().await);
        signal2.update(|values| values.0.push(5.0 + 1 as f64));
        Executor::tick().await;
        println!("{:?}",observer.recv().await);
        signal2.update(|values| values.0.push(5.0 + 2 as f64));
        Executor::tick().await;
        println!("{:?}",observer.recv().await);
        
        
    }
}
