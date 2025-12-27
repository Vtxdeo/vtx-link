use crate::engine::Engine;
use crate::state::SharedState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Response, StatusCode},
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{error, info};

pub async fn serve_hls_file(
    State(state): State<SharedState>,
    Path((stream_name, file_name)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, String)> {
    // 1. Trigger stream startup logic for .m3u8 or keep-alive logic for .ts
    if file_name.ends_with(".m3u8") {
        // Start stream if it's a .m3u8 file
        let _ = Engine::start_stream(&state, &stream_name)
            .await
            .map_err(|e| {
                // Log error if stream startup fails
                error!("Failed to auto-start stream: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;
    } else {
        // For .ts files, refresh the stream's last access time
        let mut streams = state.active_streams.lock().unwrap();
        if let Some(running) = streams.get_mut(&stream_name) {
            running.last_accessed = std::time::Instant::now();
        } else {
            // Return an error if the stream is not running
            return Err((StatusCode::NOT_FOUND, "Stream not running".to_string()));
        }
    }

    // 2. Construct the file path (reading from the configured HLS Root directory, supports RAMDisk)
    let mut file_path = PathBuf::from(&state.config.server.hls_root);
    file_path.push(&stream_name);
    file_path.push(&file_name);

    // 3. Smartly wait for the .m3u8 file to be generated (only applicable for .m3u8)
    if file_name.ends_with(".m3u8") {
        for i in 0..15 {
            // Break if the file exists
            if file_path.exists() {
                break;
            }
            if i == 0 {
                // Log that we're waiting for HLS generation
                info!("Waiting for HLS generation: {:?}", file_path);
            }
            // Wait before retrying
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    // 4. Open the file for reading
    let file = File::open(&file_path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    // 5. Determine the Content-Type based on the file extension
    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Create a stream from the file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Return the response with appropriate headers and the file content
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap())
}
