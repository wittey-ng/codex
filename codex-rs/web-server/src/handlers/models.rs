use axum::Json;
use axum::extract::State;
use codex_app_server_protocol::Model;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffortPreset;
use serde::Deserialize;
use serde::Serialize;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListModelsParams {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListModelsResponse {
    #[schema(value_type = Vec<Object>)]
    pub data: Vec<Model>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

/// GET /api/v2/models
///
/// Lists available AI models with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/api/v2/models",
    params(
        ("limit" = Option<usize>, Query, description = "Maximum number of models to return (default: 50)"),
        ("offset" = Option<usize>, Query, description = "Number of models to skip (default: 0)"),
        ("capability" = Option<String>, Query, description = "Filter by capability (e.g., 'vision')"),
        ("provider" = Option<String>, Query, description = "Filter by provider (e.g., 'anthropic', 'openai')")
    ),
    responses(
        (status = 200, description = "Models list retrieved successfully", body = ListModelsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Models"
)]
pub async fn list_models(
    State(state): State<WebServerState>,
) -> Result<Json<ListModelsResponse>, ApiError> {
    // TODO: Enable axum "query" feature for query parameters
    let params = ListModelsParams {
        limit: None,
        offset: None,
        capability: None,
        provider: None,
    };

    // List all models
    let all_models = state
        .thread_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await
        .into_iter()
        .filter(|preset| preset.show_in_picker)
        .map(model_from_preset)
        .collect::<Vec<Model>>();

    // Apply filters
    let mut filtered_models = all_models;

    if let Some(capability) = &params.capability {
        // TODO: Implement capability filtering when ModelPreset includes capability field
        tracing::warn!("Capability filtering not yet implemented: {}", capability);
    }

    if let Some(provider) = &params.provider {
        filtered_models.retain(|model| model.id.to_lowercase().contains(&provider.to_lowercase()));
    }

    let total = filtered_models.len();

    // Apply pagination
    let limit = params.limit.unwrap_or(50).min(100); // Max 100 per page
    let offset = params.offset.unwrap_or(0);

    let end = (offset + limit).min(total);
    let data = if offset < total {
        filtered_models[offset..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(Json(ListModelsResponse {
        data,
        total,
        limit,
        offset,
    }))
}

fn model_from_preset(preset: ModelPreset) -> Model {
    let ModelPreset {
        id,
        model,
        display_name,
        description,
        default_reasoning_effort,
        supported_reasoning_efforts,
        supports_personality,
        is_default,
        upgrade,
        show_in_picker,
        supported_in_api: _,
        input_modalities,
    } = preset;

    Model {
        id,
        model,
        upgrade: upgrade.map(|upgrade| upgrade.id),
        display_name,
        description,
        hidden: !show_in_picker,
        supported_reasoning_efforts: reasoning_efforts_from_preset(supported_reasoning_efforts),
        default_reasoning_effort,
        input_modalities,
        supports_personality,
        is_default,
    }
}

fn reasoning_efforts_from_preset(
    efforts: Vec<ReasoningEffortPreset>,
) -> Vec<ReasoningEffortOption> {
    efforts
        .iter()
        .map(|preset| ReasoningEffortOption {
            reasoning_effort: preset.effort,
            description: preset.description.to_string(),
        })
        .collect()
}
