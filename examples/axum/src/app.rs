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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    user: String,
    message: String,
    timestamp: u64,
}

#[component]
pub fn App() -> impl IntoView {
    // Provide websocket connection
    leptos_ws::provide_websocket();
    let count = leptos_ws::ReadOnlySignal::new("count", 0 as i32).unwrap();

    let history = leptos_ws::ReadOnlySignal::new("history", History { entries: vec![] }).unwrap();
    let count_bidirectional = leptos_ws::BiDirectionalSignal::new("count_bi", 0 as i32).unwrap();

    // Add simple echo channel signal
    let echo_channel = leptos_ws::ChannelSignal::<String>::new("echo").unwrap();
    let (echo_messages, set_echo_messages) = signal(Vec::<String>::new());
    let (echo_input, set_echo_input) = signal(String::new());

    // Set up echo callback
    let echo_messages_setter = set_echo_messages.clone();
    echo_channel.on_client(move |msg: &String| {
        echo_messages_setter.update(|messages| {
            messages.push(format!("Echo: {}", msg));
        });
    });
    echo_channel
        .on_server({
            let echo_channel = echo_channel.clone();
            move |msg: &String| {
                echo_channel.send_message(msg.to_owned()).unwrap();
            }
        })
        .ok();

    let count = move || count.get();
    let count_bi = {
        let count_bidirectional = count_bidirectional.clone();
        move || count_bidirectional.get()
    };

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
        <button on:click={let count_bi = count_bidirectional.clone(); move |_| {
            count_bi.update(move |value| *value += 1);
        }}>Increment Counter Client</button>
        <button on:click=move |_| {
            spawn_local(async move {
                update_count_bi().await.unwrap();
            });
        }>Increment Counter Server</button>
        <h1>"Count: " {count_bi}</h1>

        // Simple echo channel example
        <div style="border: 1px solid #ccc; padding: 10px; margin: 10px 0;">
            <h2>"Echo Channel Signal Example"</h2>
            <div style="height: 100px; overflow-y: auto; border: 1px solid #eee; padding: 5px; margin: 10px 0;">
                <For
                    each=move || echo_messages.get()
                    key=|msg| msg.clone()
                    children=move |msg: String| {
                        view! { <div>{msg}</div> }
                    }
                />
            </div>
            <input
                type="text"
                prop:value=move || echo_input.get()
                on:input=move |ev| {
                    set_echo_input.set(event_target_value(&ev));
                }
                placeholder="Type something to echo..."
            />
            <button on:click={
                let echo_channel = echo_channel.clone();
                let echo_input = echo_input.clone();
                let set_echo_input = set_echo_input.clone();
                move |_| {
                    let msg = echo_input.get();
                    if !msg.trim().is_empty() {
                        // Send message through channel
                        echo_channel.send_message(msg).ok();
                        set_echo_input.set(String::new());
                    }
                }
            }>"Send Echo"</button>
        </div>
    }
}

#[server]
async fn update_count() -> Result<(), ServerFnError> {
    use std::time::Duration;
    use tokio::time::sleep;
    let count = leptos_ws::ReadOnlySignal::new("count", 0 as i32).unwrap();
    for i in 0..1000 {
        count.update(move |value| *value = i);
        println!("Updated count to {}", i);
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

#[server]
async fn update_count_bi() -> Result<(), ServerFnError> {
    use std::time::Duration;
    use tokio::time::sleep;
    let count = leptos_ws::BiDirectionalSignal::new("count_bi", 0 as i32).unwrap();
    count.update(move |value| *value += 100);
    Ok(())
}
use leptos_ws::messages::Messages;

#[server]
async fn update_history() -> Result<(), ServerFnError> {
    use std::time::Duration;
    use tokio::time::sleep;
    let history = leptos_ws::ReadOnlySignal::new("history", History { entries: vec![] }).unwrap();
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
