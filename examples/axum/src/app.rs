use std::time::Duration;

use leptos::{
    prelude::*,
    spawn::{spawn_local, Executor},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HistoryEntrie {
    name: String,
    number: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct History {
    entries: Vec<HistoryEntrie>,
}

#[component]
pub fn App() -> impl IntoView {
    // Provide websocket connection
    leptos_ws::provide_websocket("http://localhost:3000/ws");
    let count = leptos_ws::ServerSignal::new("count".to_string(), 0 as i32).unwrap();
    let history =
        leptos_ws::ServerSignal::new("history".to_string(), History { entries: vec![] }).unwrap();

    let count = move || count.get();

    view! {
        <button on:click=move |_| {
            spawn_local(async move {
                update_count().await;
            });
        }>Start Counter</button>
        <h1>"Count: " {count}</h1>
        <button on:click=move |_| {
            spawn_local(async move {
                update_history().await;
            });
        }>Start History Changes</button>
        <p>{move || format!("history: {:?}",history.get())}</p>
    }
}
#[server]
async fn update_count() -> Result<(), ServerFnError> {
    use tokio::time::sleep;
    let count = leptos_ws::server_signal::ServerSignal::new("count".to_string(), 0 as i32).unwrap();
    for i in 0..1000 {
        count.update(move |value| *value = i);
        println!("{}", count.get_untracked());
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

#[server]
async fn update_history() -> Result<(), ServerFnError> {
    use tokio::time::sleep;
    let history = leptos_ws::server_signal::ServerSignal::new(
        "history".to_string(),
        History { entries: vec![] },
    )
    .unwrap();
    for i in 0..255 {
        history.update(move |value| {
            value.entries.push(HistoryEntrie {
                name: format!("{}", i).to_string(),
                number: i as u16,
            })
        });
        sleep(Duration::from_millis(1000)).await;
    }
    Ok(())
}
