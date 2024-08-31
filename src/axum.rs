use crate::{messages::Messages, messages::ServerSignalUpdate, server_signals::ServerSignals};
use axum::extract::ws::Message;
use futures::{future::BoxFuture, stream::SplitSink, SinkExt, StreamExt};
use leptos::logging::error;
use std::sync::Arc;
use tokio::{
    spawn,
    sync::{broadcast::Receiver, RwLock},
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

use axum::extract::WebSocketUpgrade;
use axum::response::Response;

pub fn websocket(
    server_signals: ServerSignals,
) -> impl Fn(WebSocketUpgrade) -> BoxFuture<'static, Response> + Clone + Send + 'static {
    move |ws: WebSocketUpgrade| {
        let value = server_signals.clone();
        Box::pin(async move { ws.on_upgrade(move |socket| handle_socket(socket, value)) })
    }
}

async fn handle_socket(socket: axum::extract::ws::WebSocket, server_signals: ServerSignals) {
    let (send, mut recv) = socket.split();
    let send = Arc::new(RwLock::new(send));
    spawn(async move {
        while let Some(message) = recv.next().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        let message: Messages = serde_json::from_str(&text).unwrap();
                        match message {
                            Messages::Establish(name) => {
                                let recv = server_signals.add_observer(name.clone()).await.unwrap();
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
                                    .await
                                    .unwrap();
                                spawn(handle_broadcasts(recv, send.clone()));
                            }
                            Messages::Update(_) => {
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
