use leptos::prelude::*;
use leptos::server_fn::{codec::JsonEncoding, BoxedStream, Websocket};
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HistoryEntry {
    name: String,
    number: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

#[component]
pub fn App() -> impl IntoView {
    // Provide websocket connection
    leptos_ws::provide_websocket();
    let count = leptos_ws::ServerSignal::new("count", 0 as i32).unwrap();

    let history = leptos_ws::ServerSignal::new("history", History { entries: vec![] }).unwrap();

    let count = move || count.get();

    view! {
        <button on:click=move |_| {
            spawn_local(async move {
                update_count().await.unwrap();
            });
        }>Start Counter</button>
        <h1>"Count: " {count}</h1>
        <button on:click=move |_| {
            spawn_local(async move {
             let _ = update_history().await.unwrap();
            });
        }>Start History Changes</button>
        <p>{move || format!("history: {:?}",history.get())}</p>
    }
}
#[server]
async fn update_count() -> Result<(), ServerFnError> {
    use std::time::Duration;
    use tokio::time::sleep;
    let count = leptos_ws::ServerSignal::new("count", 0 as i32).unwrap();
    for i in 0..1000 {
        count.update(move |value| *value = i);
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}
use leptos_ws::messages::Messages;

#[server]
async fn update_history() -> Result<(), ServerFnError> {
    use std::time::Duration;
    use tokio::time::sleep;
    let history = leptos_ws::ServerSignal::new("history", History { entries: vec![] }).unwrap();
    for i in 0..255 {
        history.update(move |value| {
            value.entries.push(HistoryEntry {
                name: format!("{}", i * 2).to_string(),
                number: i * 2 + 1 as u16,
            })
        });
        sleep(Duration::from_millis(1000)).await;
    }
    Ok(())
}
