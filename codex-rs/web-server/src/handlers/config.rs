use axum::Json;
use axum::extract::State;
use codex_app_server_protocol::*;
use codex_core::config::service::ConfigServiceError;
use codex_core::config_loader::ConfigRequirementsToml;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct WriteConfigValueRequest {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: MergeStrategy,
    pub file_path: Option<String>,
    pub expected_version: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchWriteConfigRequest {
    pub edits: Vec<ConfigEdit>,
    pub file_path: Option<String>,
    pub expected_version: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WriteConfigResponse {
    pub new_version: String,
}

impl From<ConfigServiceError> for ApiError {
    fn from(err: ConfigServiceError) -> Self {
        ApiError::InternalError(format!("Config service error: {err}"))
    }
}

/// GET /api/v2/config
///
/// Reads the effective configuration from all layers
#[utoipa::path(
    get,
    path = "/api/v2/config",
    params(
        ("include_layers" = bool, Query, description = "Include configuration layers in response")
    ),
    responses(
        (status = 200, description = "Configuration retrieved successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Configuration"
)]
pub async fn read_config(
    State(state): State<WebServerState>,
) -> Result<Json<ConfigReadResponse>, ApiError> {
    // Note: include_layers parameter not currently supported
    // TODO: Enable axum "query" feature and use Query extractor
    let params = ConfigReadParams {
        include_layers: false,
        cwd: None,
    };

    let response = state.config_service.read(params).await?;
    Ok(Json(response))
}

/// PUT /api/v2/config
///
/// Writes a single configuration value
#[utoipa::path(
    put,
    path = "/api/v2/config",
    request_body = WriteConfigValueRequest,
    responses(
        (status = 200, description = "Configuration value written successfully", body = WriteConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Version conflict"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Configuration"
)]
pub async fn write_config_value(
    State(state): State<WebServerState>,
    Json(req): Json<WriteConfigValueRequest>,
) -> Result<Json<ConfigWriteResponse>, ApiError> {
    let params = ConfigValueWriteParams {
        key_path: req.key_path,
        value: req.value,
        merge_strategy: req.merge_strategy,
        file_path: req.file_path,
        expected_version: req.expected_version,
    };

    let response = state.config_service.write_value(params).await?;
    Ok(Json(response))
}

/// PATCH /api/v2/config
///
/// Writes multiple configuration values in a batch
#[utoipa::path(
    patch,
    path = "/api/v2/config",
    request_body = BatchWriteConfigRequest,
    responses(
        (status = 200, description = "Configuration batch written successfully", body = WriteConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Version conflict"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Configuration"
)]
pub async fn batch_write_config(
    State(state): State<WebServerState>,
    Json(req): Json<BatchWriteConfigRequest>,
) -> Result<Json<ConfigWriteResponse>, ApiError> {
    let params = ConfigBatchWriteParams {
        edits: req.edits,
        file_path: req.file_path,
        expected_version: req.expected_version,
    };

    let response = state.config_service.batch_write(params).await?;
    Ok(Json(response))
}

/// GET /api/v2/config/requirements
///
/// Reads configuration requirements (allowed values, constraints)
#[utoipa::path(
    get,
    path = "/api/v2/config/requirements",
    responses(
        (status = 200, description = "Configuration requirements retrieved"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Configuration"
)]
pub async fn read_config_requirements(
    State(state): State<WebServerState>,
) -> Result<Json<Option<ConfigRequirementsToml>>, ApiError> {
    let requirements = state.config_service.read_requirements().await?;
    Ok(Json(requirements))
}
