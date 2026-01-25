use axum::Json;
use axum::extract::Multipart;
use axum::extract::Path;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header::CONTENT_DISPOSITION;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use futures::StreamExt;
use futures::TryStreamExt;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AttachmentMetadata {
    /// Attachment unique identifier
    #[schema(example = "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf")]
    pub id: String,
    /// Original filename
    #[schema(example = "image.png")]
    pub filename: String,
    /// MIME type
    #[schema(example = "image/png")]
    pub mime_type: String,
    /// File size in bytes
    #[schema(example = 1024)]
    pub size: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadResponse {
    /// Attachment unique identifier
    #[schema(example = "019bcfb9-4ea6-72e0-b43d-6b7e26ff0daf")]
    pub attachment_id: String,
    /// Original filename
    #[schema(example = "image.png")]
    pub filename: String,
    /// File size in bytes
    #[schema(example = 1024)]
    pub size: u64,
}

#[utoipa::path(
    post,
    path = "/api/v1/attachments",
    request_body(content = inline(String), content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "File uploaded successfully", body = UploadResponse),
        (status = 400, description = "Invalid request or file too large"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Attachments"
)]
pub async fn upload_attachment(
    State(state): State<WebServerState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, ApiError> {
    let attachment_id = Uuid::new_v4().to_string();

    fs::create_dir_all(&state.attachments_dir)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to create attachments dir: {e}")))?;

    let mut filename = String::from("unnamed");
    let mut mime_type = String::from("application/octet-stream");
    let mut total_size = 0u64;
    let mut file_saved = false;

    // Only accept the first file field
    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read multipart: {e}")))?
    {
        if let Some(name) = field.file_name() {
            filename = name.to_string();
        }

        if let Some(content_type) = field.content_type() {
            mime_type = content_type.to_string();
        }

        let file_path = state.attachments_dir.join(&attachment_id);
        let mut file = fs::File::create(&file_path)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to create file: {e}")))?;

        // Stream the file content to disk instead of loading into memory
        const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB limit
        let mut stream = field.into_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| ApiError::InvalidRequest(format!("Failed to read chunk: {e}")))?;

            if total_size + chunk.len() as u64 > MAX_FILE_SIZE {
                // Clean up partial file
                let _ = fs::remove_file(&file_path).await;
                return Err(ApiError::InvalidRequest(format!(
                    "File size exceeds maximum allowed size of {MAX_FILE_SIZE} bytes"
                )));
            }

            file.write_all(&chunk)
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to write file: {e}")))?;

            total_size += chunk.len() as u64;
        }

        file_saved = true;

        let metadata = AttachmentMetadata {
            id: attachment_id.clone(),
            filename: filename.clone(),
            mime_type: mime_type.clone(),
            size: total_size,
        };

        let metadata_path = state.attachments_dir.join(format!("{attachment_id}.json"));
        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| ApiError::InternalError(format!("Failed to serialize metadata: {e}")))?;

        fs::write(&metadata_path, metadata_json)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to write metadata: {e}")))?;
    }

    if !file_saved {
        return Err(ApiError::InvalidRequest(
            "No file provided in multipart request".to_string(),
        ));
    }

    Ok(Json(UploadResponse {
        attachment_id,
        filename,
        size: total_size,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/attachments/{id}",
    params(
        ("id" = String, Path, description = "Attachment ID (UUID)")
    ),
    responses(
        (status = 200, description = "File download", content_type = "application/octet-stream"),
        (status = 400, description = "Invalid attachment ID"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Attachment not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Attachments"
)]
pub async fn download_attachment(
    State(state): State<WebServerState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    // Validate ID is a valid UUID to prevent path traversal
    uuid::Uuid::parse_str(&id).map_err(|_| ApiError::AttachmentNotFound)?;

    let file_path = state.attachments_dir.join(&id);
    let metadata_path = state.attachments_dir.join(format!("{id}.json"));

    if !file_path.exists() {
        return Err(ApiError::AttachmentNotFound);
    }

    // Canonicalize and verify paths are within attachments_dir
    let canonical_file_path = file_path
        .canonicalize()
        .map_err(|_| ApiError::AttachmentNotFound)?;
    let canonical_attachments_dir = state.attachments_dir.canonicalize().map_err(|e| {
        ApiError::InternalError(format!("Failed to resolve attachments directory: {e}"))
    })?;

    if !canonical_file_path.starts_with(&canonical_attachments_dir) {
        return Err(ApiError::InvalidRequest(
            "Invalid attachment path".to_string(),
        ));
    }

    let metadata_json = fs::read_to_string(&metadata_path)
        .await
        .map_err(|_| ApiError::AttachmentNotFound)?;

    let metadata: AttachmentMetadata = serde_json::from_str(&metadata_json)
        .map_err(|e| ApiError::InternalError(format!("Failed to parse metadata: {e}")))?;

    // Stream the file instead of reading it all into memory
    let file = fs::File::open(&canonical_file_path)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to open file: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let mut response = Response::new(body);

    // Safely parse headers with fallbacks
    let content_type = metadata
        .mime_type
        .parse()
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

    // Sanitize filename to prevent header injection
    let safe_filename = metadata
        .filename
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect::<String>();
    let safe_filename = if safe_filename.is_empty() {
        "attachment".to_string()
    } else {
        safe_filename
    };

    let content_disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{safe_filename}\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment"));

    response.headers_mut().insert(CONTENT_TYPE, content_type);
    response
        .headers_mut()
        .insert(CONTENT_DISPOSITION, content_disposition);

    Ok(response)
}
