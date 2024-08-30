use std::{collections::HashMap, sync::Arc};

use crate::{
    messages::Messages,
    messages::ServerSignalUpdate,
    server_signals::{self, ServerSignals},
};
use axum::{
    async_trait,
    extract::{ws::Message, State},
};
use futures::{
    future::BoxFuture,
    stream::{select_all, SplitSink},
    Future, SinkExt, StreamExt,
};
use leptos::logging::error;
use leptos::{
    prelude::warn,
    reactive_graph::{effect::Effect, owner::expect_context},
};
use tokio::{
    spawn,
    sync::{
        broadcast::{channel, error::RecvError, Receiver, Sender},
        Mutex, RwLock,
    },
    task::JoinSet,
};

pub async fn handle_broadcasts(
    mut receiver: Receiver<ServerSignalUpdate>,
    sink: Arc<RwLock<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>,
) {
    while let Ok(message) = receiver.recv().await {
        if sink
            .write()
            .await
            .send(Message::Text(
                serde_json::to_string(&Messages::Update(message)).unwrap(),
            ))
            .await
            .is_err()
        {
            break;
        };
    }
}

pub async fn websocket(
    ws: axum::extract::WebSocketUpgrade,
    State(server_signals): State<ServerSignals>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, server_signals))
}

async fn handle_socket(mut socket: axum::extract::ws::WebSocket, server_signals: ServerSignals) {
    let (mut send, mut recv) = socket.split();
    let send = Arc::new(RwLock::new(send));
    let server_signals2 = server_signals.clone();
    spawn(async move {
        while let Some(message) = recv.next().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        let message: Messages = serde_json::from_str(&text).unwrap();
                        match message {
                            Messages::Establish(name) => {
                                let mut recv =
                                    server_signals.add_observer(name.clone()).await.unwrap();
                                send.clone()
                                    .write()
                                    .await
                                    .send(Message::Text(
                                        serde_json::to_string(&Messages::EstablishResponse((
                                            name.clone(),
                                            server_signals
                                                .json(name.clone())
                                                .await
                                                .unwrap()
                                                .unwrap(),
                                        )))
                                        .unwrap(),
                                    ))
                                    .await;
                                spawn(handle_broadcasts(recv, send.clone()));
                            }
                            Messages::Update(update) => {
                                error!("You can't change the server signal from the client side")
                            }
                            Messages::EstablishResponse(_) => {
                                error!("You can't change the server signal from the client side")
                            }
                        }
                    }
                    Message::Binary(_) => todo!(),
                    Message::Ping(_) => todo!(),
                    Message::Pong(_) => todo!(),
                    Message::Close(_) => {}
                }
            } else {
                break;
            }
        }
    })
    .await
    .unwrap();
}
