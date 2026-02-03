use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use codex_app_server_protocol::SkillDependencies;
use codex_app_server_protocol::SkillErrorInfo;
use codex_app_server_protocol::SkillInterface;
use codex_app_server_protocol::SkillMetadata;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::result::Result;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::state::WebServerState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListSkillsParams {
    #[serde(default)]
    pub cwds: Vec<String>, // Changed from PathBuf
    #[serde(default)]
    pub force_reload: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SkillsListEntry {
    pub cwd: String, // Changed from PathBuf
    #[schema(value_type = Vec<Object>)]
    pub skills: Vec<SkillMetadata>,
    #[schema(value_type = Vec<Object>)]
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListSkillsResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSkillConfigRequest {
    pub enabled: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateSkillConfigResponse {
    pub effective_enabled: bool,
}

/// GET /api/v2/skills
///
/// Lists skills available in the workspace
#[utoipa::path(
    get,
    path = "/api/v2/skills",
    params(
        ("cwds" = Option<Vec<String>>, Query, description = "Working directories to search for skills (default: current config cwd)"),
        ("force_reload" = Option<bool>, Query, description = "Force reload skills from disk (default: false)")
    ),
    responses(
        (status = 200, description = "Skills list retrieved successfully", body = ListSkillsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Skills"
)]
pub async fn list_skills(
    State(state): State<WebServerState>,
) -> Result<Json<ListSkillsResponse>, ApiError> {
    // TODO: Enable axum "query" feature for query parameters
    let params = ListSkillsParams {
        cwds: Vec::new(),
        force_reload: false,
    };
    // Get current config to determine default cwd
    let config = codex_core::config::Config::load_with_cli_overrides(vec![])
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to load config: {e}")))?;

    let cwds = if params.cwds.is_empty() {
        vec![config.cwd.clone()]
    } else {
        params.cwds.into_iter().map(PathBuf::from).collect()
    };

    let skills_manager = state.thread_manager.skills_manager();
    let mut data = Vec::new();

    for cwd in cwds {
        let outcome = skills_manager
            .skills_for_cwd(&cwd, params.force_reload)
            .await;
        let errors = errors_to_info(&outcome.errors);
        let skills = skills_to_info(&outcome.skills, &outcome.disabled_paths);

        data.push(SkillsListEntry {
            cwd: cwd.display().to_string(),
            skills,
            errors,
        });
    }

    Ok(Json(ListSkillsResponse { data }))
}

/// PATCH /api/v2/skills/:name
///
/// Updates skill configuration (enable/disable)
#[utoipa::path(
    patch,
    path = "/api/v2/skills/{name}",
    params(
        ("name" = String, Path, description = "Skill name or path")
    ),
    request_body = UpdateSkillConfigRequest,
    responses(
        (status = 200, description = "Skill configuration updated successfully", body = UpdateSkillConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Skill not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Skills"
)]
pub async fn update_skill_config(
    State(state): State<WebServerState>,
    Path(name): Path<String>,
    Json(req): Json<UpdateSkillConfigRequest>,
) -> Result<Json<UpdateSkillConfigResponse>, ApiError> {
    let path = PathBuf::from(&name);
    let edits = vec![ConfigEdit::SetSkillConfig {
        path: path.clone(),
        enabled: req.enabled,
    }];

    ConfigEditsBuilder::new(&state.codex_home)
        .with_edits(edits)
        .apply()
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to update skill settings: {e}")))?;

    // Clear skills cache after update
    state.thread_manager.skills_manager().clear_cache();

    Ok(Json(UpdateSkillConfigResponse {
        effective_enabled: req.enabled,
    }))
}

// Helper functions (adapted from app-server)

fn errors_to_info(errors: &[codex_core::skills::SkillError]) -> Vec<SkillErrorInfo> {
    errors
        .iter()
        .map(|error| SkillErrorInfo {
            path: error.path.clone(),
            message: error.message.to_string(),
        })
        .collect()
}

fn skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
    disabled_paths: &std::collections::HashSet<PathBuf>,
) -> Vec<SkillMetadata> {
    skills
        .iter()
        .map(|skill| {
            let enabled = !disabled_paths.contains(&skill.path);
            SkillMetadata {
                name: skill.name.clone(),
                description: skill.description.clone(),
                short_description: skill.short_description.clone(),
                interface: skill.interface.clone().map(|interface| SkillInterface {
                    display_name: interface.display_name,
                    short_description: interface.short_description,
                    icon_small: interface.icon_small,
                    icon_large: interface.icon_large,
                    brand_color: interface.brand_color,
                    default_prompt: interface.default_prompt,
                }),
                dependencies: skill.dependencies.clone().map(|deps| SkillDependencies {
                    tools: deps
                        .tools
                        .iter()
                        .map(|tool| codex_app_server_protocol::SkillToolDependency {
                            r#type: tool.r#type.clone(),
                            value: tool.value.clone(),
                            description: tool.description.clone(),
                            transport: tool.transport.clone(),
                            command: tool.command.clone(),
                            url: tool.url.clone(),
                        })
                        .collect(),
                }),
                path: skill.path.clone(),
                scope: skill.scope.into(),
                enabled,
            }
        })
        .collect()
}
