use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ws::ChannelContext;
use leptos_ws::ChannelSignal;
use leptos_ws::provide_websocket;
use leptos_ws::traits::ChannelHandler;
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
pub struct Blocked(Arc<dashmap::DashSet<String>>);

impl Blocked {
    pub fn new() -> Self {
        Self(Arc::new(dashmap::DashSet::new()))
    }

    pub fn is_blocked(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    pub fn block(&self, id: &str) {
        self.0.insert(id.to_string());
    }

    pub fn unblock(&self, id: &str) {
        self.0.remove(id);
    }
}

/// Per-connection state tracked on the server.
#[derive(Default)]
pub struct ConnectionState {
    pub greetings_sent: usize,
    pub max_greetings: usize,
}

/// Handler struct that implements the `ChannelHandler` trait.
/// Gets `&mut ChannelContext<ConnectionState>` + the message.
struct GreetHandler {
    sender: ChannelSignal<String, ConnectionState>,
}

impl ChannelHandler<String, ConnectionState> for GreetHandler {
    fn handle(&self, ctx: &mut ChannelContext<'_, ConnectionState>, msg: &String) {
        ctx.state_mut().greetings_sent += 1;
        let max = ctx.state().max_greetings;
        let sent = ctx.state().greetings_sent;
        if sent < max {
            self.sender.send_message(msg.clone());
        }

        info!(
            "server handler: client={}, msg={}, sent={}/{}",
            ctx.client_id(),
            msg,
            sent,
            max,
        );
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HistoryEntry {
    pub name: String,
    pub number: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

static VISITOR_COUNT: AtomicUsize = AtomicUsize::new(0);

#[component]
pub fn App() -> impl IntoView {
    provide_websocket();

    let greet_channel = ChannelSignal::<String, ConnectionState>::new("greet").unwrap();
    let (messages, set_messages) = signal(Vec::<String>::new());
    let (input, set_input) = signal(String::new());
    let (blocked_input, set_blocked_input) = signal(String::new());

    #[cfg(feature = "ssr")]
    {
        let blocked = use_context::<Blocked>().unwrap();
        let handler = GreetHandler {
            sender: greet_channel.clone(),
        };

        // Send mapper: filter outgoing messages per-connection state
        greet_channel
            .add_send_mapper(
                move |ctx: &mut ChannelContext<'_, ConnectionState>, msg: &String| {
                    if blocked.is_blocked(ctx.client_id()) {
                        None
                    } else {
                        Some(format!("[{}] {}", ctx.state().greetings_sent, msg))
                    }
                },
            )
            .ok();

        greet_channel.on_server(handler).ok();
    }

    // ── client-side handler ──
    let messages_setter = set_messages.clone();
    greet_channel.on_client(move |msg: &String| {
        messages_setter.update(|msgs| msgs.push(msg.clone()));
    });

    // ── UI ──
    view! {
        <h1>"Advanced Channel Example"</h1>

        <div style="border:1px solid #ccc;padding:10px;margin:10px 0;">
            <h2>"Greet Channel"</h2>
            <p>"Per-connection max 5 greetings, then messages are suppressed via send mapper."</p>
            <div style="height:150px;overflow-y:auto;border:1px solid #eee;padding:5px;">
                <For each=move || messages.get() key=|m| m.clone() children=|msg| view! { <div>{msg}</div> }/>
            </div>
            <input type="text" prop:value=move || input.get()
                on:input=move |ev| set_input.set(event_target_value(&ev))
                placeholder="Type a greeting..."/>
            <button on:click={
                let greet_channel = greet_channel.clone();
                let input = input.clone();
                let set_input = set_input.clone();
                move |_| {
                    let msg = input.get();
                    if !msg.trim().is_empty() {
                        greet_channel.send_message(msg).ok();
                        set_input.set(String::new());
                    }
                }
            }>"Send via Broadcast"</button>
        </div>

        <div style="border:1px solid #ccc;padding:10px;margin:10px 0;">
            <h2>"Manage Blocklist"</h2>
            <p>"Enter a client ID to add to the blocklist."</p>
            <input type="text" prop:value=move || blocked_input.get()
                on:input=move |ev| set_blocked_input.set(event_target_value(&ev))
                placeholder="client-id"/>
            <button on:click={
                let blocked_input = blocked_input.clone();
                let set_blocked_input = set_blocked_input.clone();
                move |_| {
                    let id = blocked_input.get();
                    if !id.trim().is_empty() {
                        info!("would block client_id={}", id);
                        set_blocked_input.set(String::new());
                        spawn_local(async move {
                            block_client(id).await;
                        });
                    }
                }
            }>"Block Client"</button>
        </div>
    }
}

#[server]
pub async fn block_client(id: String) -> Result<(), ServerFnError> {
    let blocked_clients = use_context::<Blocked>().unwrap();
    blocked_clients.block(&id);
    Ok(())
}
