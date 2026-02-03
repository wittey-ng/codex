use axum::Json;
use axum::extract::State;
use codex_app_server_protocol::*;
use codex_core::auth::CodexAuth;
use codex_protocol::account::PlanType;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

// TODO: Full authentication implementation requires:
// 1. Integration with codex-login crate for OAuth flow
// 2. Login server management (spawn/shutdown)
// 3. Account login state tracking
// 4. Token refresh mechanism
//
// Reference implementations:
// - app-server/src/codex_message_processor.rs (login/logout handlers)
// - app-server/src/login_manager.rs (OAuth flow management)

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum LoginRequest {
    #[serde(rename = "apiKey")]
    ApiKey {
        #[serde(rename = "apiKey")]
        api_key: String,
    },
    #[serde(rename = "chatgpt")]
    Chatgpt,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LoginResponse {
    #[serde(rename = "apiKey")]
    ApiKey {},
    #[serde(rename = "chatgpt")]
    Chatgpt { login_id: String, auth_url: String },
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelLoginRequest {
    pub login_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CancelLoginResponse {
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {}

#[derive(Debug, Serialize)]
pub struct GetAccountResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Serialize)]
pub struct GetRateLimitsResponse {
    pub rate_limits: RateLimitSnapshot,
}

/// POST /api/v2/auth/login
///
/// Initiates login flow for API Key or ChatGPT OAuth
#[utoipa::path(
    post,
    path = "/api/v2/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login initiated successfully", body = LoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn login(
    State(_state): State<WebServerState>,
    Json(_req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    // TODO: Implement login flow
    // - For API Key: Store in auth.json via AuthManager
    // - For ChatGPT: Spawn login server, generate OAuth URL, return login_id
    //
    // Reference: app-server/src/codex_message_processor.rs::handle_login_account
    Err(ApiError::InternalError(
        "Login endpoint not yet implemented".to_string(),
    ))
}

/// POST /api/v2/auth/login/cancel
///
/// Cancels an in-progress ChatGPT OAuth login
#[utoipa::path(
    post,
    path = "/api/v2/auth/login/cancel",
    request_body = CancelLoginRequest,
    responses(
        (status = 200, description = "Login cancelled", body = CancelLoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Login ID not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn cancel_login(
    State(_state): State<WebServerState>,
    Json(_req): Json<CancelLoginRequest>,
) -> Result<Json<CancelLoginResponse>, ApiError> {
    // TODO: Implement cancel login
    // - Lookup login_id in active login sessions
    // - Shutdown login server for that session
    // - Return status (Canceled or NotFound)
    //
    // Reference: app-server/src/codex_message_processor.rs::handle_cancel_login_account
    Err(ApiError::InternalError(
        "Cancel login endpoint not yet implemented".to_string(),
    ))
}

/// POST /api/v2/auth/logout
///
/// Logs out the current user
#[utoipa::path(
    post,
    path = "/api/v2/auth/logout",
    responses(
        (status = 200, description = "Logged out successfully", body = LogoutResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn logout(State(state): State<WebServerState>) -> Result<Json<LogoutResponse>, ApiError> {
    // Clear auth.json via AuthManager
    let auth = state.auth_manager.auth().await;
    if auth.is_some() {
        // TODO: Implement proper logout
        // - Delete auth.json file
        // - Clear cached auth in AuthManager
        // - Emit account/updated notification via SSE
        //
        // Reference: app-server/src/codex_message_processor.rs::handle_logout_account
        Err(ApiError::InternalError(
            "Logout endpoint not yet implemented".to_string(),
        ))
    } else {
        Ok(Json(LogoutResponse {}))
    }
}

/// GET /api/v2/auth/account
///
/// Returns current account information
#[utoipa::path(
    get,
    path = "/api/v2/auth/account",
    responses(
        (status = 200, description = "Account information retrieved"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn get_account(
    State(state): State<WebServerState>,
) -> Result<Json<GetAccountResponse>, ApiError> {
    let auth = state.auth_manager.auth().await;

    let account = auth.and_then(|auth| match auth {
        CodexAuth::ApiKey(_) => Some(Account::ApiKey {}),
        CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
            let email = auth.get_account_email()?;
            let plan_type = auth.account_plan_type().unwrap_or(PlanType::Free);
            Some(Account::Chatgpt { email, plan_type })
        }
    });

    let requires_openai_auth = account.is_none();

    Ok(Json(GetAccountResponse {
        account,
        requires_openai_auth,
    }))
}

/// GET /api/v2/auth/rate-limits
///
/// Returns current account rate limits
#[utoipa::path(
    get,
    path = "/api/v2/auth/rate-limits",
    responses(
        (status = 200, description = "Rate limits retrieved"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn get_rate_limits(
    State(_state): State<WebServerState>,
) -> Result<Json<GetRateLimitsResponse>, ApiError> {
    // TODO: Implement rate limits retrieval
    // - Fetch from backend API using auth token
    // - Return RateLimitSnapshot
    //
    // Reference: app-server/src/codex_message_processor.rs::handle_get_account_rate_limits
    Err(ApiError::InternalError(
        "Rate limits endpoint not yet implemented".to_string(),
    ))
}
