use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;

use crate::default_client::build_reqwest_client;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_api::Provider as ApiProvider;
use codex_protocol::models::FunctionCallOutputContentItem;

pub struct GenerateImageHandler;

#[derive(Deserialize)]
struct GenerateImageArgs {
    prompt: String,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default = "default_quality")]
    quality: String,
    #[serde(default = "default_n")]
    n: u8,
}

fn default_size() -> String {
    "1024x1024".to_string()
}

fn default_quality() -> String {
    "standard".to_string()
}

fn default_n() -> u8 {
    1
}

#[derive(Serialize)]
struct DallERequest {
    model: String,
    prompt: String,
    n: u8,
    size: String,
    quality: String,
    response_format: String,
}

#[derive(Deserialize)]
struct DallEResponse {
    data: Vec<ImageData>,
}

#[derive(Deserialize)]
struct ImageData {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    b64_json: Option<String>,
}

#[async_trait]
impl ToolHandler for GenerateImageHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "generate_image handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GenerateImageArgs = parse_arguments(&arguments)?;

        let codex_config = invocation.turn.client.config();
        let provider = super::openai_provider_for_tools(&codex_config)?;
        let api_provider = super::openai_api_provider(&provider)?;
        let api_key = super::resolve_openai_api_key(invocation.turn.as_ref(), &provider).await?;
        let client = build_reqwest_client();

        match generate_image_dalle(&args, &api_provider, &api_key, &client).await {
            Ok(content_items) => {
                let count = content_items.len();
                Ok(ToolOutput::Function {
                    content: format!("Generated {count} image(s) successfully"),
                    content_items: Some(content_items),
                    success: Some(true),
                })
            }
            Err(e) => Err(FunctionCallError::RespondToModel(format!(
                "Failed to generate image: {e}"
            ))),
        }
    }
}

async fn generate_image_dalle(
    args: &GenerateImageArgs,
    api_provider: &ApiProvider,
    api_key: &str,
    client: &Client,
) -> Result<Vec<FunctionCallOutputContentItem>, Box<dyn std::error::Error + Send + Sync>> {
    let request = DallERequest {
        model: "dall-e-3".to_string(),
        prompt: args.prompt.clone(),
        n: args.n,
        size: args.size.clone(),
        quality: args.quality.clone(),
        response_format: "b64_json".to_string(),
    };

    let response = client
        .post(api_provider.url_for_path("images/generations"))
        .headers(api_provider.headers.clone())
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("OpenAI API error: {error_text}").into());
    }

    let dalle_response: DallEResponse = response.json().await?;

    let mut content_items = Vec::new();
    for (idx, image_data) in dalle_response.data.into_iter().enumerate() {
        if let Some(b64_data) = image_data.b64_json {
            content_items.push(FunctionCallOutputContentItem::InputImage {
                image_url: format!("data:image/png;base64,{b64_data}"),
            });
        } else if let Some(url) = image_data.url {
            let image_bytes = client.get(&url).send().await?.bytes().await?;
            let b64_data = general_purpose::STANDARD.encode(&image_bytes);
            content_items.push(FunctionCallOutputContentItem::InputImage {
                image_url: format!("data:image/png;base64,{b64_data}"),
            });
        } else {
            tracing::warn!("Image {idx} has no data");
        }
    }

    Ok(content_items)
}
