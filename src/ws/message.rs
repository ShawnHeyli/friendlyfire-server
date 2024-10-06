use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum WsMessage {
    Media(MediaMessage),
    ClientCount(ClientCountMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MediaMessage {
    media_url: String, // Actually a tauri::Url but we don't need the real type here
    top_message: String,
    bottom_message: String,
    sender: User,
    timeout: u64, // in milliseconds
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct User {
    username: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ClientCountMessage {
    client_count: u64,
}

impl ClientCountMessage {
    pub fn new(client_count: u64) -> Self {
        Self { client_count }
    }
}