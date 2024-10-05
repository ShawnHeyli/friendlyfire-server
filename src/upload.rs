use axum::response::IntoResponse;
use rand::distributions::DistString;
use std::{net::SocketAddr, time::Duration};

use axum::{body::Bytes, extract::ConnectInfo, http::StatusCode};
use rand::distributions::Alphanumeric;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    time::sleep,
};

pub async fn upload(ConnectInfo(addr): ConnectInfo<SocketAddr>, body: Bytes) -> impl IntoResponse {
    println!("{} accessed /upload", addr);

    let folder_path = std::path::Path::new("uploads");
    // Check if the folder already exists
    if !folder_path.exists() {
        // Create the folder
        match fs::create_dir(folder_path).await {
            Ok(_) => println!("Folder 'uploads' created successfully."),
            Err(e) => println!("Failed to create folder 'uploads': {}", e),
        }
    }

    let filename = Alphanumeric.sample_string(&mut rand::thread_rng(), 24);
    match File::create(format!("uploads/{}", &filename)).await {
        Ok(mut file) => {
            if let Err(e) = file.write_all(&body).await {
                eprintln!("Failed to write file: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to write file".to_string(),
                );
            }
            // Delete the file after 2min
            let filename_clone = filename.clone();
            tokio::spawn(async move {
                sleep(Duration::from_secs(120)).await;
                if let Err(e) = fs::remove_file(format!("uploads/{}", &filename_clone)).await {
                    eprintln!("Unable to delete file: {:?}", e);
                } else {
                    println!("File deleted: {}", &filename_clone);
                }
            });

            (StatusCode::OK, filename)
        }
        Err(e) => {
            eprintln!("Failed to create file: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create file".to_string(),
            )
        }
    }
}
