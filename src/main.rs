mod upload;
mod uploads;
mod ws;

use axum::{
    extract::{ws::Message, ConnectInfo, DefaultBodyLimit},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use queue::MediaProcessor;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let app_state = Arc::new(AppState::default());

    let media_processor = MediaProcessor::new();

    tokio::spawn(async move {
        media_processor.process_messages().await.unwrap();
    });

    let routes = Router::new()
        .route("/", get(|| async { Html::from("Video Sync Server") }))
        .route("/ws", get(ws::ws_handler).with_state(app_state.clone()))
        .route("/healthcheck", get(healthcheck))
        .route(
            "/upload",
            post(upload::upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/uploads/:asset", get(uploads::serve_asset))
        .into_make_service_with_connect_info::<SocketAddr>();

    match tokio::net::TcpListener::bind("0.0.0.0:7331").await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, routes).await {
                eprintln!("Server error: {:?}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to bind to address: {:?}", e);
        }
    }
}

struct AppState {
    sender: broadcast::Sender<Message>,
}

impl Default for AppState {
    fn default() -> Self {
        let (sender, _receiver) = broadcast::channel(32);
        AppState { sender }
    }
}

async fn healthcheck(ConnectInfo(addr): ConnectInfo<SocketAddr>) -> impl IntoResponse {
    println!("{} accessed /healthcheck", addr);
    (StatusCode::OK, "Ok")
}
