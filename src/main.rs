#![feature(async_closure)]

use std::net::SocketAddr;
use futures::{sink::SinkExt, stream::StreamExt};
use axum::{routing::get, extract::ws::{WebSocket, WebSocketUpgrade, Message}, Router, response::IntoResponse};


async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws)
}

async fn handle_ws(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        loop {
            if let Some(m) = rx.recv().await {
                if sender.send(m).await.is_err() {
                    return;
                }
            }
        }
    });

    if let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(m) => {
                if tx.send(Message::Text(m)).is_err() {
                    return;
                }
            },
            Message::Binary(m) => {
                if tx.send(Message::Binary(m)).is_err() {
                    return;
                }
            },
            _ => (),
        }
    }
}

#[tokio::main]
async fn main() {
    let router = Router::new()
        .route("/", get(async || {
            let content = tokio::fs::read("../assets/index.html").await.unwrap();
            String::from_utf8(content).unwrap()
        }))
        .route("/ws", get(ws_handler));
    
    let addr = SocketAddr::from(([0, 0, 0, 0], 8030));

    let server = axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to await for SIGINT")
        });
    
    server.await.expect("Failed to start server");
}
