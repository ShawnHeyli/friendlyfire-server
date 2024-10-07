mod queue;
mod upload;
mod uploads;
mod ws;

use axum::{
    extract::{ws::Message, DefaultBodyLimit},
    response::Html,
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use ws::{ws_handler, WsMessage};

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState::new());
    let queue_tx = state.sender.clone();
    let mut queue_rx = state.timed_queue.subscribe();

    tokio::spawn(async move {
        loop {
            if let Ok(msg) = queue_rx.recv().await {
                let ws_message =
                    Message::Text(serde_json::to_string(&WsMessage::Media(msg)).unwrap());
                if let Err(e) = queue_tx.send(ws_message) {
                    eprintln!("Failed to send message to websocket clients: {:?}", e);
                }
            }
        }
    });

    let app = app(state);

    match tokio::net::TcpListener::bind("0.0.0.0:7331").await {
        Ok(listener) => {
            if let Err(e) = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
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
    timed_queue: queue::TimedQueue<ws::MediaMessage>,
}

impl AppState {
    fn new() -> Self {
        Self {
            sender: broadcast::channel(16).0,
            timed_queue: queue::TimedQueue::new(),
        }
    }
}

fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { Html::from("Video Sync Server") }))
        .route("/ws", get(ws_handler).with_state(state))
        .route("/healthcheck", get(|| async { Html::from("Ok") }))
        .route(
            "/upload",
            post(upload::upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/uploads/:asset", get(uploads::serve_asset))
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use axum::{body, extract::connect_info::MockConnectInfo, http, response::IntoResponse};
    use futures_util::StreamExt;
    use http_body_util::BodyExt;
    use tokio_tungstenite::tungstenite;
    use tower::ServiceExt;
    use ws::ClientCountMessage;
    use ws::WsMessage;

    use super::*;

    #[tokio::test]
    async fn test_root() {
        let state = Arc::new(AppState::new());
        let app = app(state);

        let req = http::Request::get("/").body(body::Body::empty()).unwrap();
        let response = app
            .oneshot(req)
            .await
            .expect("Failed to call endpoint")
            .into_response();

        assert_eq!(response.status(), http::StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Video Sync Server");
    }

    #[tokio::test]
    async fn test_healthcheck() {
        let state = Arc::new(AppState::new());
        let app = app(state);

        let req = http::Request::get("/healthcheck")
            .body(body::Body::empty())
            .unwrap();
        let response = app
            .oneshot(req)
            .await
            .expect("Failed to call endpoint")
            .into_response();

        assert_eq!(response.status(), http::StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Ok");
    }

    #[tokio::test]
    async fn test_404() {
        let state = Arc::new(AppState::new());
        let app = app(state);

        let req = http::Request::get("/notfound")
            .body(body::Body::empty())
            .unwrap();
        let response = app
            .oneshot(req)
            .await
            .expect("Failed to call endpoint")
            .into_response();

        assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_upload() {
        let state = Arc::new(AppState::new());
        let app = app(state)
            .layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 3000))))
            .into_service();

        let fake_image: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let req = http::Request::post("/upload")
            .body(body::Body::from(fake_image))
            .unwrap();

        let response = app
            .oneshot(req)
            .await
            .expect("Failed to call endpoint")
            .into_response();

        assert_eq!(response.status(), http::StatusCode::OK);
        // We should receive a remote path (24 random letters) for the uploaded file
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.len(), 24);

        // CLEANUP: Remove the uploaded file {uploads/remote_path}
        let path = format!("uploads/{}", String::from_utf8(body.to_vec()).unwrap());
        std::fs::remove_file(path).unwrap();
    }

    // Check if the websocket endpoint is reachable and we receive a switch protocol response
    #[tokio::test]
    async fn test_ws_connection() {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(AppState::new());

        let app = app(state)
            .layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 3000))))
            .into_service();

        tokio::spawn(async move {
            axum::serve(
                listener,
                axum::ServiceExt::into_make_service_with_connect_info::<SocketAddr>(app),
            )
            .await
            .unwrap();
        });

        let (mut socket, _response) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .unwrap();

        let msg = match socket.next().await.unwrap().unwrap() {
            tungstenite::Message::Text(text) => {
                let msg: WsMessage = serde_json::from_str(&text).unwrap();
                msg
            }
            other => panic!("expected a text message but got {other:?}"),
        };

        assert_eq!(msg, WsMessage::ClientCount(ClientCountMessage::new(1)));
    }
}
