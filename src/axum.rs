use crate::{
    messages::{Messages, ServerSignalMessage, ServerSignalUpdate},
    server_signals::ServerSignals,
};
use axum::extract::ws::Message;
use futures::{future::BoxFuture, stream::SplitSink, SinkExt, StreamExt};
use leptos::logging::error;
use std::sync::Arc;
use tokio::{
    spawn,
    sync::{broadcast::Receiver, RwLock},
};

async fn handle_broadcasts(
    mut receiver: Receiver<ServerSignalUpdate>,
    sink: Arc<RwLock<SplitSink<axum::extract::ws::WebSocket, axum::extract::ws::Message>>>,
) {
    while let Ok(message) = receiver.recv().await {
        if sink
            .write()
            .await
            .send(Message::Text(
                serde_json::to_string(&Messages::ServerSignal(ServerSignalMessage::Update(
                    message,
                )))
                .unwrap()
                .into(),
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
/// Creates a WebSocket handler function for upgrading HTTP connections to WebSocket connections.
///
/// This function returns a closure that can be used as a route handler in an Axum web server to handle
/// WebSocket upgrade requests. It sets up the necessary infrastructure to manage WebSocket
/// connections and integrate them with the server's signaling system.
///
/// # Arguments
///
/// * `server_signals` - A `ServerSignals` instance that provides access to server-wide
///   communication channels and state.
///
/// # Returns
///
/// Returns an implementation of a function that:
/// - Takes a `WebSocketUpgrade` as an argument
/// - Returns a `BoxFuture<'static, Response>`
/// - Is `Clone`, `Send`, and has a `'static` lifetime
///
/// The returned function handles the WebSocket upgrade process and delegates the actual
/// WebSocket communication to the `handle_socket` function.
///
/// # Example
///
/// ```
/// use axum::Router;
/// use axum::routing::{get, post};
///
/// let app = Router::new()
///     .route("/api/*fn_name", post(server_fn_handler))
///     .route(
///         "/ws",
///         get(leptos_ws::axum::websocket(state.server_signals.clone())),
///     )
///     .leptos_routes_with_handler(routes, get(leptos_routes_handler))
///     .fallback(file_and_error_handler)
///     .with_state(state);
/// ```
///
/// In this example, the `websocket` function is used to create a WebSocket handler for the "/ws" route
/// in an Axum router configuration.
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
    let _ = spawn(async move {
        while let Some(message) = recv.next().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => {
                        if let Ok(message) = serde_json::from_str::<Messages>(&text) {
                            match message {
                                Messages::ServerSignal(server_msg) => match server_msg {
                                    ServerSignalMessage::Establish(name) => {
                                        let recv = server_signals
                                            .add_observer(name.clone())
                                            .await
                                            .unwrap();
                                        send.clone()
                                            .write()
                                            .await
                                            .send(Message::Text(
                                                serde_json::to_string(&Messages::ServerSignal(
                                                    ServerSignalMessage::EstablishResponse((
                                                        name.clone(),
                                                        server_signals
                                                            .json(name.clone())
                                                            .await
                                                            .unwrap()
                                                            .unwrap(),
                                                    )),
                                                ))
                                                .unwrap()
                                                .into(),
                                            ))
                                            .await
                                            .unwrap();
                                        spawn(handle_broadcasts(recv, send.clone()));
                                    }
                                    _ => error!("Unexpected server signal message from client"),
                                },
                            }
                        } else {
                            leptos::logging::error!("Error transmitting message")
                        }
                    }
                    Message::Binary(_) => todo!(),
                    Message::Ping(_) => send
                        .clone()
                        .write()
                        .await
                        .send(Message::Pong(vec![1, 2, 3].into()))
                        .await
                        .unwrap(),
                    Message::Pong(_) => todo!(),
                    Message::Close(_) => {}
                }
            } else {
                break;
            }
        }
    })
    .await;
}
