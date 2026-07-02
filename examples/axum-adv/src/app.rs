use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ws::ChannelContext;
use leptos_ws::ChannelSignal;
use leptos_ws::provide_websocket;
use leptos_ws::traits::ChannelHandler;
use log::info;
use serde::{Deserialize, Serialize};

static TOPICS: &[&str] = &["sports", "tech", "finance", "health"];

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Messages {
    Subscribe(String),
    Unsubscribe(String),
    Event { topic: String, content: String },
}

/// Per-connection state: tracks which topics this client is interested in.
#[derive(Default)]
pub struct ConnectionState {
    pub interests: Vec<String>,
}

/// Handler: client subscribes/unsubscribes to topics.
struct SubscribeHandler;

impl ChannelHandler<Messages, ConnectionState> for SubscribeHandler {
    fn handle(&self, ctx: &mut ChannelContext<'_, ConnectionState>, msg: &Messages) {
        match msg {
            Messages::Subscribe(topic) => {
                if !ctx.state().interests.contains(topic) {
                    ctx.state_mut().interests.push(topic.clone());
                    info!("client {} subscribed to '{}'", ctx.client_id(), topic);
                }
            }
            Messages::Unsubscribe(topic) => {
                ctx.state_mut().interests.retain(|t| t != topic);
                info!("client {} unsubscribed from '{}'", ctx.client_id(), topic);
            }
            Messages::Event { .. } => {}
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_websocket();

    let channel = ChannelSignal::<Messages, ConnectionState>::new("updates").unwrap();

    let (messages, set_messages) = signal(Vec::<String>::new());
    let (event_topic, set_event_topic) = signal(String::new());
    let (event_msg, set_event_msg) = signal(String::new());

    #[cfg(feature = "ssr")]
    {
        channel.on_server(SubscribeHandler).ok();

        channel
            .add_send_mapper_ref(
                move |ctx: &ChannelContext<'_, ConnectionState>, msg: &Messages| {
                    match msg {
                        Messages::Event { topic, content: _ } => {
                            if ctx.state().interests.iter().any(|i| i == topic) {
                                Some(msg.clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                },
            )
            .ok();
    }

    let messages_setter = set_messages.clone();
    channel.on_client(move |msg: &Messages| {
        match msg {
            Messages::Event { topic, content } => {
                messages_setter.update(|msgs| msgs.push(format!("[{}] {}", topic, content)));
            }
            _ => {}
        }
    });

    view! {
        <h1>"Interest-Based Event Distribution"</h1>
        <p>
            "Subscribe to topics below. When the server broadcasts an event for a topic, "
            "only clients subscribed to that topic receive it."
        </p>

        <div style="display:flex;gap:20px;">

            <div style="border:1px solid #ccc;padding:10px;flex:1;">
                <h2>"Client"</h2>
                <p>"Toggle topics to subscribe/unsubscribe:"</p>
                {TOPICS.iter().map(|topic| {
                    let channel = channel.clone();
                    let topic = *topic;
                    view! {
                        <label>
                            <input type="checkbox"
                                on:change=move |ev| {
                                    if event_target_checked(&ev) {
                                        channel.send_message(Messages::Subscribe(topic.to_string())).ok();
                                    } else {
                                        channel.send_message(Messages::Unsubscribe(topic.to_string())).ok();
                                    }
                                }
                            />
                            { topic }
                        </label>
                        <br/>
                    }
                }).collect_view()}

                <h3>"Received Events"</h3>
                <div style="height:200px;overflow-y:auto;border:1px solid #eee;padding:5px;">
                    <For
                        each=move || messages.get()
                        key=|m| m.clone()
                        children=|msg| view! { <div>{msg}</div> }
                    />
                </div>
            </div>

            <div style="border:1px solid #ccc;padding:10px;flex:1;">
                <h2>"Server (broadcast event)"</h2>
                <p>"Send an event for a topic. Only subscribed clients will see it."</p>
                <select
                    prop:value=move || event_topic.get()
                    on:change=move |ev| set_event_topic.set(event_target_value(&ev))
                >
                    <option value="">"-- select topic --"</option>
                    {TOPICS.iter().map(|t| view! {
                        <option value=t.to_string()>{t.to_string()}</option>
                    }).collect_view()}
                </select>
                <br/><br/>
                <input type="text"
                    prop:value=move || event_msg.get()
                    on:input=move |ev| set_event_msg.set(event_target_value(&ev))
                    placeholder="Event content..."
                />
                <button on:click={
                    move |_| {
                        let topic = event_topic.get();
                        let msg = event_msg.get();
                        if !topic.is_empty() && !msg.is_empty() {
                            spawn_local(async move {
                                send_event(topic, msg).await;
                            });
                            set_event_msg.set(String::new());
                        }
                    }
                }>"Send Event"</button>
            </div>

        </div>
    }
}

#[server]
pub async fn send_event(topic: String, content: String) -> Result<(), ServerFnError> {
    let channel = ChannelSignal::<Messages, ConnectionState>::new("updates")
        .map_err(|_| ServerFnError::new("failed to get channel"))?;
    channel
        .send_message(Messages::Event { topic, content })
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}
