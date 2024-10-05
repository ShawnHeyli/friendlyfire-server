use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::{ConnectInfo, Path},
    http::StatusCode,
    response::IntoResponse,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub async fn serve_asset(
    Path(path): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    println!("{} accessed /uploads/{}", addr, path);
    let path = format!("uploads/{}", path);
    match File::open(path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            (StatusCode::OK, body)
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Body::from("File not found".to_string()),
        ),
    }
}
