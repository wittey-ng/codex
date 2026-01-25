use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use codex_app_server_protocol::McpServerStatus;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use tokio::sync::oneshot;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListMcpServerStatusParams {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListMcpServerStatusResponse {
    #[schema(value_type = Vec<Object>)]
    pub data: Vec<McpServerStatus>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpServerRefreshResponse {}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpOAuthLoginResponse {
    pub auth_url: Option<String>,
}

/// GET /api/v2/mcp/servers
///
/// Lists MCP server status with tools, resources, and auth status
#[utoipa::path(
    get,
    path = "/api/v2/mcp/servers",
    params(
        ("limit" = Option<usize>, Query, description = "Maximum number of servers to return (default: 20)"),
        ("cursor" = Option<String>, Query, description = "Pagination cursor (offset as string)")
    ),
    responses(
        (status = 200, description = "MCP server status list retrieved successfully", body = ListMcpServerStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "MCP"
)]
pub async fn list_mcp_server_status(
    State(_state): State<WebServerState>,
) -> Result<Json<ListMcpServerStatusResponse>, ApiError> {
    // TODO: Enable axum "query" feature for query parameters
    let params = ListMcpServerStatusParams {
        limit: None,
        cursor: None,
    };
    // Spawn async task to avoid blocking
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = list_mcp_server_status_task(params).await;
        let _ = tx.send(result);
    });

    let response = rx
        .await
        .map_err(|_| ApiError::InternalError("MCP status task failed".to_string()))??;

    Ok(Json(response))
}

async fn list_mcp_server_status_task(
    params: ListMcpServerStatusParams,
) -> Result<ListMcpServerStatusResponse, ApiError> {
    // Load core config for MCP snapshot collection
    let config = codex_core::config::Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    // Collect MCP snapshot (async operation)
    let snapshot = codex_core::mcp::collect_mcp_snapshot(&config).await;

    // Group tools by server
    let tools_by_server = codex_core::mcp::group_tools_by_server(&snapshot.tools);

    // Collect all unique server names
    let mut server_names: Vec<String> = config
        .mcp_servers
        .keys()
        .cloned()
        .chain(snapshot.auth_statuses.keys().cloned())
        .chain(snapshot.resources.keys().cloned())
        .chain(snapshot.resource_templates.keys().cloned())
        .collect();
    server_names.sort();
    server_names.dedup();

    // Apply pagination
    let limit = params.limit.unwrap_or(100);
    let effective_limit = limit.clamp(1, 100);

    let cursor_offset = params
        .cursor
        .as_deref()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    let start = cursor_offset;
    let total = server_names.len();

    // If start offset is beyond total, return empty list
    if start >= total {
        return Ok(ListMcpServerStatusResponse {
            data: Vec::new(),
            next_cursor: None,
        });
    }

    let end = start.saturating_add(effective_limit).min(total);

    // Build McpServerStatus list for the current page
    let data: Vec<McpServerStatus> = server_names[start..end]
        .iter()
        .map(|name| McpServerStatus {
            name: name.clone(),
            tools: tools_by_server.get(name).cloned().unwrap_or_default(),
            resources: snapshot.resources.get(name).cloned().unwrap_or_default(),
            resource_templates: snapshot
                .resource_templates
                .get(name)
                .cloned()
                .unwrap_or_default(),
            auth_status: snapshot
                .auth_statuses
                .get(name)
                .cloned()
                .unwrap_or(codex_protocol::protocol::McpAuthStatus::Unsupported)
                .into(),
        })
        .collect();

    // Compute next cursor
    let next_cursor = if end < total {
        Some(end.to_string())
    } else {
        None
    };

    Ok(ListMcpServerStatusResponse { data, next_cursor })
}

/// POST /api/v2/mcp/servers/refresh
///
/// Refreshes MCP server configuration
#[utoipa::path(
    post,
    path = "/api/v2/mcp/servers/refresh",
    responses(
        (status = 200, description = "MCP servers refreshed successfully", body = McpServerRefreshResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "MCP"
)]
pub async fn refresh_mcp_servers(
    State(_state): State<WebServerState>,
) -> Result<Json<McpServerRefreshResponse>, ApiError> {
    // TODO: Implement MCP server refresh
    // This requires:
    // 1. Loading latest config
    // 2. Serializing MCP servers
    // 3. Creating RefreshConfig (need to check if this type exists)
    // 4. Calling ThreadManager::refresh_mcp_servers()
    //
    // Reference: app-server/src/codex_message_processor.rs::mcp_server_refresh

    // For now, return success stub
    Ok(Json(McpServerRefreshResponse {}))
}

/// POST /api/v2/mcp/servers/:name/auth
///
/// Initiates OAuth login for an MCP server
#[utoipa::path(
    post,
    path = "/api/v2/mcp/servers/{name}/auth",
    params(
        ("name" = String, Path, description = "MCP server name")
    ),
    responses(
        (status = 200, description = "OAuth login initiated", body = McpOAuthLoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "MCP server not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "MCP"
)]
pub async fn mcp_oauth_login(
    State(_state): State<WebServerState>,
    Path(name): Path<String>,
) -> Result<Json<McpOAuthLoginResponse>, ApiError> {
    // Load config to get MCP server settings
    let config = codex_core::config::Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    // Get MCP server configuration
    let server = config
        .mcp_servers
        .get()
        .get(&name)
        .ok_or_else(|| ApiError::NotFound(format!("MCP server not found: {name}")))?;

    // Extract transport details (OAuth only supported for StreamableHttp)
    let (url, http_headers, env_http_headers) = match &server.transport {
        codex_core::config::types::McpServerTransportConfig::StreamableHttp {
            url,
            http_headers,
            env_http_headers,
            ..
        } => (url.clone(), http_headers.clone(), env_http_headers.clone()),
        _ => {
            return Err(ApiError::InvalidRequest(
                "OAuth login is only supported for streamable HTTP servers".to_string(),
            ));
        }
    };

    // Perform OAuth login and get authorization URL
    let handle = codex_rmcp_client::perform_oauth_login_return_url(
        &name,
        &url,
        config.mcp_oauth_credentials_store_mode,
        http_headers,
        env_http_headers,
        &[],  // scopes - default to empty for now (could be extended via request body)
        None, // timeout_secs - use default
        config.mcp_oauth_callback_port,
    )
    .await
    .map_err(|e| ApiError::InternalError(format!("Failed to initiate OAuth login: {e}")))?;

    let authorization_url = handle.authorization_url().to_string();

    // Spawn background task to wait for OAuth completion
    // TODO: Send McpServerOauthLoginCompletedNotification via SSE when available
    // For now, we just wait in the background without sending notifications
    let notification_name = name.clone();
    tokio::spawn(async move {
        let (success, error) = match handle.wait().await {
            Ok(()) => {
                tracing::info!(
                    "MCP OAuth login completed successfully for: {}",
                    notification_name
                );
                (true, None)
            }
            Err(err) => {
                tracing::error!("MCP OAuth login failed for {}: {}", notification_name, err);
                (false, Some(err.to_string()))
            }
        };

        // TODO: Send McpServerOauthLoginCompletedNotification via SSE
        // This requires SSE integration which will be implemented later
        // Notification structure:
        // {
        //   name: notification_name,
        //   success,
        //   error
        // }
        tracing::debug!(
            "MCP OAuth login completion event (notification pending SSE): name={}, success={}, error={:?}",
            notification_name,
            success,
            error
        );
    });

    Ok(Json(McpOAuthLoginResponse {
        auth_url: Some(authorization_url),
    }))
}
