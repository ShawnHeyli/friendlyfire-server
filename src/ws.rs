use std::{net::SocketAddr, sync::Arc};

use axum::extract::{
    ws::{Message, WebSocket},
    ConnectInfo, State, WebSocketUpgrade,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Sender;

use crate::AppState;

#[derive(Serialize, Deserialize, Debug)]
pub enum WsMessage {
    Media(MediaMessage),
    ClientCount(ClientCountMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MediaMessage {
    media_url: String, // Actually a tauri::Url but we don't need the real type here
    top_message: String,
    bottom_message: String,
    sender: User,
    timeout: u64, // in milliseconds
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct User {
    username: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ClientCountMessage {
    client_count: u64,
}

fn handle_client_connect(tx: Sender<Message>) {
    let client_count = tx.receiver_count() as u64;
    let message = WsMessage::ClientCount(ClientCountMessage { client_count });
    if let Err(e) = tx.send(Message::Text(serde_json::to_string(&message).unwrap())) {
        eprintln!("Failed to send client count update: {:?}", e);
    }
}

fn handle_client_disconnect(tx: Sender<Message>) {
    let client_count = (tx.receiver_count() - 1) as u64;
    let message = WsMessage::ClientCount(ClientCountMessage { client_count });
    if let Err(e) = tx.send(Message::Text(serde_json::to_string(&message).unwrap())) {
        eprintln!("Failed to send client count update: {:?}", e);
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl axum::response::IntoResponse {
    println!("{} accessed /ws", addr);
    ws.on_upgrade(move |socket| handle_socket(socket, app_state, addr))
}

async fn handle_socket(mut socket: WebSocket, app_state: Arc<AppState>, _addr: SocketAddr) {
    let mut rx = app_state.sender.subscribe();
    let tx = app_state.sender.clone();

    handle_client_connect(tx.clone());

    loop {
        tokio::select! {
            msg= socket.recv() => {
                if let Some(Ok(msg)) = msg{
                    match msg{
                        Message::Text(msg) => {
                            println!("Received socket: {:?}", &msg);
                            if let Err(e) = tx.send(Message::Text(msg)) {
                                eprintln!("Failed to send message: {:?}", e);
                            }
                        },
                        Message::Binary(msg) =>
                            println!("Received socket: {:?}", &msg),
                        Message::Ping(msg) =>
                            println!("Received socket: {:?}", &msg),
                        Message::Pong(msg) =>
                            println!("Received socket: {:?}", &msg),
                        Message::Close(msg) => {
                            println!("Received socket: {:?}", &msg);
                            handle_client_disconnect(tx.clone());
                            drop(rx);
                            drop(tx);
                            break;
                        }
                    }
                } else {
                    handle_client_disconnect(tx.clone());
                }
            },

            msg = rx.recv() => {
                if let Ok(msg) = msg {
                    println!("Received channel: {:?}", &msg);
                    if let Err(e) = socket.send(msg.clone()).await {
                        println!("Failed to send message: {:?}", e);
                        handle_client_disconnect(tx.clone());
                        break;
                    }
                    println!("Sent socket: {:?}", &msg);
                }
            }
        }
    }
}
