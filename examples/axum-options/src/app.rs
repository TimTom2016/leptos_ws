use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ws::WsSignals;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator as _;

#[derive(
    Serialize,
    Deserialize,
    Clone,
    strum::Display,
    strum::EnumIter,
    strum::AsRefStr,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
)]
pub enum Options {
    Name,
    Email,
    Phone,
}

impl Options {
    pub fn as_data(&self) -> Data {
        match self {
            Options::Name => Data::Name(String::new()),
            Options::Email => Data::Email(String::new()),
            Options::Phone => Data::Phone(String::new()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Data {
    Name(String),
    Email(String),
    Phone(String),
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::Name(v) => write!(f, "{}", v),
            Data::Email(v) => write!(f, "{}", v),
            Data::Phone(v) => write!(f, "{}", v),
        }
    }
}

pub struct OptionsStore {
    handler: RwSignal<Option<leptos_ws::ReadOnlySignal<Data>>>,
    selected: RwSignal<Options>,
}

impl Default for OptionsStore {
    fn default() -> Self {
        let handler = RwSignal::new(None);
        let selected = RwSignal::new(Options::Name);
        Self { handler, selected }
    }
}

impl OptionsStore {
    pub fn select(&self, option: Options) -> Option<()> {
        let mut signals = use_context::<WsSignals>()?;
        let selected = self.selected.get_untracked();
        #[cfg(feature = "hydrate")]
        // Cleans up the previously selected, and to unsubscribe from server and not get updates from those "channels"
        signals.delete_signal(selected.as_ref());
        let new_handler = leptos_ws::ReadOnlySignal::new(option.as_ref(), option.as_data()).ok()?;
        self.selected.set(option);
        self.handler.set(Some(new_handler));
        Some(())
    }

    #[track_caller]
    pub fn get_handler(&self) -> Option<leptos_ws::ReadOnlySignal<Data>> {
        self.handler.get()
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provide websocket connection
    leptos_ws::provide_websocket();
    #[cfg(feature = "hydrate")]
    {
        use leptos_ws::ServerSignalWebSocket;
        let context = expect_context::<ServerSignalWebSocket>();
        context.set_on_disconnect(move || {
            leptos::logging::error!("WebSocket disconnected");
        });
        context.set_on_reconnect(move || {
            leptos::logging::warn!("WebSocket reconnected");
        });
        context.set_on_connect(move || {
            leptos::logging::warn!("WebSocket connected");
        });
    }
    #[cfg(feature = "ssr")]
    {
        ///!!! IMPORTANT: Signals need to exist on server before first establih from client side, because of this we create all possible signals
        for option in Options::iter() {
            leptos_ws::ReadOnlySignal::new(option.as_ref(), option.as_data()).ok();
        }
    }
    let store = StoredValue::new(OptionsStore::default());
    let (selected, set_selected) = signal(Options::Name);
    Effect::new(move || {
        let selected = selected.get();
        store.read_value().select(selected);
    });
    let (name_input, set_name_input) = signal(String::new());
    let (email_input, set_email_input) = signal(String::new());
    let (phone_input, set_phone_input) = signal(String::new());

    view! {
        <div>
            <label for="option-select">"Select option: "</label>
            <select
                id="option-select"
                on:change=move |ev| {
                    let value = event_target_value(&ev);
                    let option = match value.as_str() {
                        "Name" => Options::Name,
                        "Email" => Options::Email,
                        "Phone" => Options::Phone,
                        _ => Options::Name,
                    };
                    set_selected.set(option);
                }
                prop:value=move || selected.get().to_string()
            >
                <option value="Name">"Name"</option>
                <option value="Email">"Email"</option>
                <option value="Phone">"Phone"</option>
            </select>
        </div>

        <div>
            <strong>"Current value: "</strong>
            <span>{move || {
                let _ = selected.get();
                match store.read_value().get_handler() {
                    Some(handler) => handler.get().to_string(),
                    _ => Default::default(),
                }
            }}</span>
        </div>

        <div>
            <h3>"Name"</h3>
            <input
                type="text"
                prop:value=move || name_input.get()
                on:input=move |ev| set_name_input.set(event_target_value(&ev))
                placeholder="Enter name"
            />
            <button on:click=move |_| {
                let value = name_input.get_untracked();
                spawn_local(async move {
                    let _ = set_name(value).await;
                });
            }>"Set Name"</button>
        </div>

        <div>
            <h3>"Email"</h3>
            <input
                type="text"
                prop:value=move || email_input.get()
                on:input=move |ev| set_email_input.set(event_target_value(&ev))
                placeholder="Enter email"
            />
            <button on:click=move |_| {
                let value = email_input.get_untracked();
                spawn_local(async move {
                    let _ = set_email(value).await;
                });
            }>"Set Email"</button>
        </div>

        <div>
            <h3>"Phone"</h3>
            <input
                type="text"
                prop:value=move || phone_input.get()
                on:input=move |ev| set_phone_input.set(event_target_value(&ev))
                placeholder="Enter phone"
            />
            <button on:click=move |_| {
                let value = phone_input.get_untracked();
                spawn_local(async move {
                    let _ = set_phone(value).await;
                });
            }>"Set Phone"</button>
        </div>
    }
}

#[server]
async fn set_name(name: String) -> Result<(), ServerFnError> {
    let option = Options::Name;
    let signal = leptos_ws::ReadOnlySignal::new(option.as_ref(), option.as_data())?;
    signal.update(move |data| *data = Data::Name(name));
    Ok(())
}

#[server]
async fn set_email(email: String) -> Result<(), ServerFnError> {
    let option = Options::Email;
    let signal = leptos_ws::ReadOnlySignal::new(option.as_ref(), option.as_data())?;
    signal.update(move |data| *data = Data::Email(email));
    Ok(())
}

#[server]
async fn set_phone(phone: String) -> Result<(), ServerFnError> {
    let option = Options::Phone;
    let signal = leptos_ws::ReadOnlySignal::new(option.as_ref(), option.as_data())?;
    signal.update(move |data| *data = Data::Phone(phone));
    Ok(())
}
