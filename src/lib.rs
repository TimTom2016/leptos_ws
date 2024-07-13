use std::{any::{Any, TypeId}, borrow::Cow};
use json_patch::Patch;
use serde::{Serialize,Deserialize};
use serde_json::Value;
use wasm_bindgen::JsValue;
use web_sys::WebSocket;

pub mod error;



#[cfg(feature="ssr")]
pub mod server_signals;

#[cfg(target_arch="wasm32")]
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


#[cfg(target_arch="wasm32")]
impl ServerSignalWebSocket {
    /// Returns the inner websocket.
    pub fn ws(&self) -> WebSocket {
        self.ws.clone()
    }
}


#[cfg(all(feature="axum",feature="ssr"))]
mod axum;


#[cfg(target_arch="wasm32")]
#[inline]
fn provide_websocket_inner(url: &str) -> Result<Option<WebSocket>, JsValue> {
    use web_sys::MessageEvent;
    use wasm_bindgen::{prelude::Closure, JsCast};
    use leptos::{use_context, SignalUpdate};
    use js_sys::{Function, JsString};

    if use_context::<ServerSignalWebSocket>().is_none() {
        let ws = WebSocket::new(url)?;
        provide_context(ServerSignalWebSocket { ws, state_signals: Rc::default(), delayed_updates: Rc::default() });
    }

    let ws = use_context::<ServerSignalWebSocket>().unwrap();

    let handlers = ws.state_signals.clone();
    let delayed_updates = ws.delayed_updates.clone();

    let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        let ws_string = event.data().dyn_into::<JsString>().unwrap().as_string().unwrap();
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
                delayed_map.entry(name.clone()).or_default().push(update_signal.patch.clone());
            }
        }
    }) as Box<dyn FnMut(_)>);
    let function: &Function = callback.as_ref().unchecked_ref();
    ws.ws.set_onmessage(Some(function));

    // Keep the closure alive for the lifetime of the program
    callback.forget();

    Ok(Some(ws.ws()))
}




#[cfg(not(target_arch="wasm32"))]
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
    pub fn new_from_json<T>(name: impl Into<Cow<'static, str>>, old: &Value, new: &Value) -> Self {
        let patch = json_patch::diff(old, new);
        ServerSignalUpdate {
            name: name.into(),
            patch,
        }
    }
    
    pub fn is<T: 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }

    pub fn downcast<T: 'static>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if (*self).is::<T>() {
            let ptr = Box::into_raw(self) as *mut T;
            // SAFETY: Keep reading :-)
            unsafe { Ok(Box::from_raw(ptr)) }
        } else {
            Err(self)
        }
    }
}


mod test {
    use leptos::{create_action, create_effect, create_runtime, current_runtime, provide_context, Effect, RwSignal, SignalGet, SignalSet, SignalWith};

    use crate::{axum::ServerSignal, server_signals::{self, ServerSignals}};

    #[tokio::test]
    pub async fn does_server_signal_work() {
        let runtime = create_runtime();
    
        let mut server_signals = ServerSignals::new();
        let new_signal = RwSignal::new(10);

        server_signals.create_signal(new_signal).await;
        new_signal.set(22);
        assert_eq!(22,server_signals.get_signal::<RwSignal<i32>>().await.unwrap().get())
    } 
    
    #[tokio::test]
    pub async fn server_signal_test() {

        let runtime = create_runtime();
    
        let mut server_signals = ServerSignals::new();
        provide_context(server_signals);
        let signal = ServerSignal::new(RwSignal::new(0)).await.unwrap();
        let signal2 = signal.clone();
        Effect::new_isomorphic(move |_| {
            signal2.track();
            println!("Hello")
        });
        signal.set(22)
        
    } 
}