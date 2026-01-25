use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde_json::json;
use utoipa::ToSchema;

#[derive(Debug, ToSchema)]
#[schema(example = json!({"error": "Unauthorized", "status": 401}))]
pub enum ApiError {
    Unauthorized,
    #[allow(dead_code)]
    NotFound(String),
    InvalidRequest(String),
    InternalError(String),
    ThreadNotFound,
    AttachmentNotFound,
    Timeout(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::ThreadNotFound => (StatusCode::NOT_FOUND, "Thread not found".to_string()),
            ApiError::AttachmentNotFound => {
                (StatusCode::NOT_FOUND, "Attachment not found".to_string())
            }
            ApiError::Timeout(msg) => (StatusCode::GATEWAY_TIMEOUT, msg),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::InternalError(err.to_string())
    }
}
