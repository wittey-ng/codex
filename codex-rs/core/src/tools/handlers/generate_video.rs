use async_trait::async_trait;
use reqwest::Client;
use reqwest::multipart;
use serde::Deserialize;

use crate::default_client::build_reqwest_client;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_api::Provider as ApiProvider;

pub struct GenerateVideoHandler;

const SORA_2_RESOLUTIONS: [&str; 2] = ["720x1280", "1280x720"];
const SORA_2_PRO_RESOLUTIONS: [&str; 4] = ["720x1280", "1280x720", "1024x1792", "1792x1024"];

#[derive(Deserialize)]
struct GenerateVideoArgs {
    prompt: String,
    #[serde(default = "default_duration")]
    duration: u32,
    #[serde(default = "default_resolution")]
    resolution: String,
    #[serde(default = "default_model")]
    model: String,
}

fn default_duration() -> u32 {
    4
}

fn default_resolution() -> String {
    "720x1280".to_string()
}

fn default_model() -> String {
    "sora-2".to_string()
}

#[derive(Deserialize)]
struct VideoResponse {
    id: String,
    status: String,
    #[serde(default)]
    progress: Option<u32>,
}

#[async_trait]
impl ToolHandler for GenerateVideoHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "generate_video handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GenerateVideoArgs = parse_arguments(&arguments)?;

        let valid_models = ["sora-2", "sora-2-pro"];
        if !valid_models.contains(&args.model.as_str()) {
            return Err(FunctionCallError::RespondToModel(
                "generate_video model must be one of: sora-2, sora-2-pro".to_string(),
            ));
        }

        let valid_durations = [4, 8, 12];
        if !valid_durations.contains(&args.duration) {
            return Err(FunctionCallError::RespondToModel(
                "generate_video duration must be one of: 4, 8, 12".to_string(),
            ));
        }

        let valid_resolutions = if args.model == "sora-2" {
            SORA_2_RESOLUTIONS.as_slice()
        } else {
            SORA_2_PRO_RESOLUTIONS.as_slice()
        };

        if !valid_resolutions.contains(&args.resolution.as_str()) {
            return Err(FunctionCallError::RespondToModel(
                "generate_video resolution must be one of: 720x1280, 1280x720 (sora-2) or 1024x1792, 1792x1024 (sora-2-pro)"
                    .to_string(),
            ));
        }

        let codex_config = invocation.turn.client.config();
        let provider = super::openai_provider_for_tools(&codex_config)?;
        let api_provider = super::openai_api_provider(&provider)?;
        let api_key = super::resolve_openai_api_key(invocation.turn.as_ref(), &provider).await?;
        let client = build_reqwest_client();

        match generate_video_sora(&args, &api_provider, &api_key, &client).await {
            Ok(video_info) => {
                let VideoGenerationInfo {
                    id,
                    status,
                    message,
                } = video_info;
                let duration = args.duration;
                let resolution = &args.resolution;
                let model = &args.model;
                Ok(ToolOutput::Function {
                    content: format!(
                        "Video generation initiated successfully.\n\nID: {id}\nStatus: {status}\nModel: {model}\nDuration: {duration}s\nResolution: {resolution}\n{message}"
                    ),
                    content_items: None,
                    success: Some(true),
                })
            }
            Err(e) => Err(FunctionCallError::RespondToModel(format!(
                "Failed to generate video: {e}"
            ))),
        }
    }
}

struct VideoGenerationInfo {
    id: String,
    status: String,
    message: String,
}

async fn generate_video_sora(
    args: &GenerateVideoArgs,
    api_provider: &ApiProvider,
    api_key: &str,
    client: &Client,
) -> Result<VideoGenerationInfo, Box<dyn std::error::Error + Send + Sync>> {
    let form = multipart::Form::new()
        .text("model", args.model.clone())
        .text("prompt", args.prompt.clone())
        .text("seconds", args.duration.to_string())
        .text("size", args.resolution.clone());

    let response = client
        .post(api_provider.url_for_path("videos"))
        .headers(api_provider.headers.clone())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("OpenAI Sora API error: {error_text}").into());
    }

    let sora_response: VideoResponse = response.json().await?;

    let message = if let Some(progress) = sora_response.progress {
        format!("Progress: {progress}%")
    } else {
        "Video is being processed.".to_string()
    };

    Ok(VideoGenerationInfo {
        id: sora_response.id,
        status: sora_response.status,
        message,
    })
}
