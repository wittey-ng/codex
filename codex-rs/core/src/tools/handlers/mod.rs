pub mod apply_patch;
pub(crate) mod collab;
mod dynamic;
mod grep_files;
mod list_dir;
mod mcp;
mod mcp_resource;
mod plan;
mod query_vector_db;
mod read_file;
mod request_user_input;
mod shell;
mod test_sync;
mod unified_exec;
mod view_image;

pub use plan::PLAN_TOOL;
use serde::Deserialize;

use crate::auth::read_openai_api_key_from_env;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::function_tool::FunctionCallError;
use crate::model_provider_info::ModelProviderInfo;
pub use apply_patch::ApplyPatchHandler;
use codex_api::Provider as ApiProvider;
use codex_app_server_protocol::AuthMode;
pub use collab::CollabHandler;
pub use dynamic::DynamicToolHandler;
pub use grep_files::GrepFilesHandler;
pub use list_dir::ListDirHandler;
pub use mcp::McpHandler;
pub use mcp_resource::McpResourceHandler;
pub use plan::PlanHandler;
pub use query_vector_db::QueryVectorDbHandler;
pub use read_file::ReadFileHandler;
pub use request_user_input::RequestUserInputHandler;
pub use shell::ShellCommandHandler;
pub use shell::ShellHandler;
pub use test_sync::TestSyncHandler;
pub use unified_exec::UnifiedExecHandler;
pub use view_image::ViewImageHandler;

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
    })
}

fn openai_provider_for_tools(config: &Config) -> Result<ModelProviderInfo, FunctionCallError> {
    config
        .model_providers
        .get("openai")
        .cloned()
        .ok_or_else(|| {
            FunctionCallError::RespondToModel("OpenAI provider is not configured".to_string())
        })
}

fn openai_api_provider(provider: &ModelProviderInfo) -> Result<ApiProvider, FunctionCallError> {
    provider
        .to_api_provider(Some(AuthMode::ApiKey))
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

async fn resolve_openai_api_key(
    turn: &TurnContext,
    provider: &ModelProviderInfo,
) -> Result<String, FunctionCallError> {
    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(token);
    }

    if let Some(api_key) = provider
        .api_key()
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?
    {
        return Ok(api_key);
    }

    if let Some(auth_manager) = turn.client.get_auth_manager()
        && let Some(auth) = auth_manager.auth().await
        && matches!(auth.mode, AuthMode::ApiKey)
    {
        return auth
            .get_token()
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()));
    }

    if let Some(api_key) = read_openai_api_key_from_env() {
        return Ok(api_key);
    }

    let message =
        "OpenAI API key required for this tool. Run `codex login --api-key` or set OPENAI_API_KEY."
            .to_string();
    Err(FunctionCallError::RespondToModel(message))
}
