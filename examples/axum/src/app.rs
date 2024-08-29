use std::time::Duration;

use leptos::{
    prelude::*,
    spawn::{spawn_local, Executor},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Count {
    pub value: i32,
}

#[component]
pub fn App() -> impl IntoView {
    // Provide websocket connection
    leptos_ws::provide_websocket("http://localhost:3000/ws");
    #[cfg(not(feature = "ssr"))]
    let count = leptos_ws::client_signal::ClientSignal::new("count".to_string(), 0 as i32).unwrap();
    #[cfg(feature = "ssr")]
    let count = leptos_ws::server_signal::ServerSignal::new("count".to_string(), 0 as i32).unwrap();
    let count = move || count.get();

    view! {
        <button on:click=move |_| {
            spawn_local(async move {
                update_count().await;
            });
        }>Start Counter</button>
        <h1>"Count: " {count}</h1>
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
