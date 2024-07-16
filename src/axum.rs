use std::{collections::HashMap, sync::{Arc}};

use axum::{async_trait, extract::{ws::Message, State}};
use futures::{future::BoxFuture, stream::{select_all, SplitSink}, Future, SinkExt, StreamExt};
use leptos::reactive_graph::{effect::Effect, owner::expect_context};
use tokio::{spawn, sync::{broadcast::{channel, error::RecvError, Receiver, Sender}, Mutex, RwLock}, task::JoinSet};
use crate::{messages::Messages, server_signals::{self, ServerSignals}, ServerSignalUpdate};

pub async fn handle_broadcasts(mut receiver: Receiver<ServerSignalUpdate>, sink: Sender<Message>) {
    while let Ok(message) = receiver.recv().await {
        if sink.send(Message::Text(serde_json::to_string(&message).unwrap())).is_err() {
            break;
        };
    }
}

pub async fn websocket(ws: axum::extract::WebSocketUpgrade,State(server_signals): State<ServerSignals>) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket,server_signals))
}

async fn handle_socket(mut socket: axum::extract::ws::WebSocket,server_signals: ServerSignals) {
    let (mut send,mut recv) = socket.split();
    let (websocket_sender,websocket_receiver) = channel(10);
    let server_signals2 = server_signals.clone();
    spawn(async move {
        
        while let Some(message) = recv.next().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        let message: Messages = serde_json::from_str(&text).unwrap();
                        match message {
                            Messages::Establish(name) => {
                                let mut recv = server_signals.add_observer(name.clone()).await.unwrap();
                               spawn(handle_broadcasts(recv, websocket_sender.clone()));

                            },
                            Messages::Update(update) => {server_signals.update(update.name.to_string(), update).await.unwrap();},
                        }
                    },
                    Message::Binary(_) => todo!(),
                    Message::Ping(_) => todo!(),
                    Message::Pong(_) => todo!(),
                    Message::Close(_) => todo!(),
                }
            } else {
                break;
            }
            
        };
        
    }).await.unwrap();
}