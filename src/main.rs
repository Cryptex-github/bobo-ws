#![feature(async_closure)]

use std::net::SocketAddr;
use futures::{sink::SinkExt, stream::StreamExt};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum::{routing::get, extract::ws::{WebSocket, WebSocketUpgrade, Message}, Router, response::{IntoResponse, Html}};


async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws)
}

async fn handle_ws(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    
    let sender = tokio::spawn(async move {
        loop {
            if let Some(m) = rx.recv().await {
                for tries in 1..10 {
                    if sender.send(m.clone()).await.is_err() {
                        if tries == 10 {
                            return;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(tries * 2)).await;
                        continue;
                    }
                    break;
                }
            }
        }
    });
    
    while !sender.is_finished() {
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
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let router = Router::new()
        .route("/", get(async || {
            Html(std::include_str!("../assets/index.html"))
        }))
        .route("/ws", get(ws_handler))
        .layer(TraceLayer::new_for_http());
    
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
