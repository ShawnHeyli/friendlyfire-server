use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::extract::{
    ws::{Message, WebSocket},
    ConnectInfo, State, WebSocketUpgrade,
};
use tokio::sync::broadcast::Sender;

use crate::AppState;

use super::{ClientCountMessage, WsMessage};

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
                    println!("Received socket: {:?}", &msg);
                    match msg{
                        Message::Text(msg) => {
                            // Parse the message
                            match serde_json::from_str::<WsMessage>(&msg){
                                Ok(message) => {
                                    match message{
                                        WsMessage::Media(media_message) => {
                                            // Add the message to the queue
                                            println!("Adding media message to queue: {:?}", &media_message);
                                            app_state.timed_queue.add(media_message.clone(), Duration::from_millis(media_message.timeout())).await;
                                        }
                                        WsMessage::ClientCount(message) => {
                                            if let Err(e) = tx.send(Message::Text(serde_json::to_string(&message).unwrap())){
                                                eprintln!("Failed to send client count update: {:?}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse message: {:?}", e);
                                }
                            }
                        },
                        Message::Close(_msg) => {
                            handle_client_disconnect(tx.clone());
                            break;
                        }
                        _ => {}
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

fn handle_client_connect(tx: Sender<Message>) {
    let client_count = tx.receiver_count() as u64;
    let message = WsMessage::ClientCount(ClientCountMessage::new(client_count));
    if let Err(e) = tx.send(Message::Text(serde_json::to_string(&message).unwrap())) {
        eprintln!("Failed to send client count update: {:?}", e);
    }
}

fn handle_client_disconnect(tx: Sender<Message>) {
    let client_count = (tx.receiver_count() - 1) as u64;
    let message = WsMessage::ClientCount(ClientCountMessage::new(client_count));
    if let Err(e) = tx.send(Message::Text(serde_json::to_string(&message).unwrap())) {
        eprintln!("Failed to send client count update: {:?}", e);
    }
}
