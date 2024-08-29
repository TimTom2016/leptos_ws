#[cfg(not(feature = "ssr"))]
use client_signals::ClientSignals;
use json_patch::Patch;
use leptos::*;
use messages::{Messages, ServerSignalUpdate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
};
use wasm_bindgen::JsValue;
use web_sys::WebSocket;

pub mod error;
pub mod messages;
#[cfg(feature = "ssr")]
pub mod server_signal;

#[cfg(feature = "ssr")]
pub mod server_signals;

#[cfg(not(feature = "ssr"))]
pub mod client_signal;

#[cfg(not(feature = "ssr"))]
pub mod client_signals;

#[cfg(all(feature = "axum", feature = "ssr"))]
pub mod axum;
#[cfg(not(feature = "ssr"))]
#[derive(Clone)]
pub struct ServerSignalWebSocket {
    ws: send_wrapper::SendWrapper<WebSocket>,
    // References to these are kept by the closure for the callback
    // onmessage callback on the websocket
    state_signals: ClientSignals,
    // When the websocket is first established, the leptos may not have
    // completed the traversal that sets up all of the state signals.
    // Without that, we don't have a base state to apply the patches to,
    // and therefore we must keep a record of the patches to apply after
    // the state has been set up.
    delayed_updates: Arc<Mutex<HashMap<String, Vec<ServerSignalUpdate>>>>,
    delayed_msgs: Arc<Mutex<Vec<Messages>>>,
}
#[cfg(not(feature = "ssr"))]
impl ServerSignalWebSocket {
    /// Returns the inner websocket.
    pub fn ws(&self) -> WebSocket {
        self.ws.clone().take()
    }
    pub fn send(&self, msg: &Messages) -> Result<(), serde_json::Error> {
        let serialized_msg = serde_json::to_string(&msg)?;
        if self.ws.ready_state() != WebSocket::OPEN {
            self.delayed_msgs.lock().unwrap().push(msg.clone());
        } else {
            self.ws
                .send_with_str(&serialized_msg)
                .expect("Failed to send message");
        }
        Ok(())
    }
}

#[cfg(not(feature = "ssr"))]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<Option<WebSocket>, JsValue> {
    use std::time::Duration;

    use leptos::prelude::{provide_context, use_context};
    use prelude::{set_timeout, warn};
    use wasm_bindgen::{prelude::Closure, JsCast};
    use web_sys::js_sys::{Function, JsString};
    use web_sys::MessageEvent;

    if let None = use_context::<ServerSignalWebSocket>() {
        let ws = send_wrapper::SendWrapper::new(WebSocket::new(url)?);
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        provide_context(ServerSignalWebSocket {
            ws,
            state_signals: ClientSignals::new(),
            delayed_updates: Arc::default(),
            delayed_msgs: Arc::default(),
        });
    }

    match use_context::<ServerSignalWebSocket>() {
        Some(ws) => {
            let handlers = ws.state_signals.clone();
            provide_context(ws.state_signals.clone());
            let delayed_updates = ws.delayed_updates.clone();

            let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
                let ws_string = event
                    .data()
                    .dyn_into::<JsString>()
                    .unwrap()
                    .as_string()
                    .unwrap();

                match serde_json::from_str::<Messages>(&ws_string) {
                    Ok(Messages::Establish(_)) => todo!(),
                    Ok(Messages::Update(update)) => {
                        let name = &update.name;
                        let mut delayed_map = (*delayed_updates).lock().unwrap();
                        if let Some(delayed_patches) = delayed_map.remove(&name.to_string()) {
                            for patch in delayed_patches {
                                handlers.update(name.to_string(), patch);
                            }
                        }
                        handlers.update(name.to_string(), update);
                        // delayed_map
                        //     .entry(name.clone())
                        //     .or_default()
                        //     .push(update_signal.patch.clone());
                    }
                    Err(err) => {
                        warn!("Couldn't deserialize Socket Message {}", err)
                    }
                }
            }) as Box<dyn FnMut(_)>);
            let mut ws2 = ws.clone();
            let onopen_callback = Closure::<dyn FnMut()>::new(move || {
                if let Ok(mut delayed_msgs) = ws2.delayed_msgs.lock() {
                    for msg in delayed_msgs.drain(..) {
                        if let Err(err) = ws2.send(&msg) {
                            eprintln!("Failed to send delayed message: {:?}", err);
                        }
                    }
                }
            });
            let function: &Function = callback.as_ref().unchecked_ref();
            ws.ws.set_onmessage(Some(function));
            ws.ws
                .set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();
            // Keep the closure alive for the lifetime of the program
            callback.forget();

            Ok(Some(ws.ws()))
        }
        None => todo!(),
    }
}

#[cfg(feature = "ssr")]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<Option<WebSocket>, JsValue> {
    use wasm_bindgen::JsValue;
    use web_sys::WebSocket;

    Ok(None)
}

pub fn provide_websocket(url: &str) -> Result<Option<WebSocket>, JsValue> {
    provide_websocket_inner(url)
}

/// A server signal update containing the signal type name and json patch.
///
/// This is whats sent over the websocket, and is used to patch the signal if the type name matches.
#[cfg(feature = "ssr")]
#[cfg(test)]
mod test {
    use std::{time::Duration, vec};

    use any_spawner::Executor;
    use leptos::prelude::*;
    use tokio::{spawn, task, time::sleep};
    use web_sys::js_sys::Math::sign;
    #[derive(Clone, Serialize, Deserialize)]
    pub struct History(pub Vec<f64>);

    impl Into<ArcRwSignal<History>> for History {
        fn into(self) -> ArcRwSignal<History> {
            ArcRwSignal::new(self)
        }
    }

    use crate::{
        error,
        server_signal::{ServerSignal, ServerSignalTrait},
        server_signals::{self, ServerSignals},
        History,
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
        let signal = ServerSignal::new(history.clone(), History(vec![1.0, 2.0, 3.0, 4.0])).unwrap();
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
        let signal = ServerSignal::new(history.clone(), History(vec![1.0, 2.0, 3.0, 4.0])).unwrap();
        let mut observer = signal.add_observer().await;
        let signal2 = signal.clone();
        signal2.update(|values| values.0.push(5.0));
        Executor::tick().await;
        println!("{:?}", observer.recv().await);
        signal2.update(|values| values.0.push(5.0 + 1 as f64));
        Executor::tick().await;
        println!("{:?}", observer.recv().await);
        signal2.update(|values| values.0.push(5.0 + 2 as f64));
        Executor::tick().await;
        println!("{:?}", observer.recv().await);
    }
}
