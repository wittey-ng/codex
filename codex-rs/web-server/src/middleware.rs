use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;
use crate::state::WebServerState;

pub async fn auth_middleware(
    State(state): State<WebServerState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == state.auth_token {
                Ok(next.run(request).await)
            } else {
                Err(ApiError::Unauthorized)
            }
        }
        _ => Err(ApiError::Unauthorized),
    }
}
