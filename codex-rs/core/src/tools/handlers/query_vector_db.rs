use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;

use crate::config::VectorDbConfig;
use crate::default_client::build_reqwest_client;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use codex_api::Provider as ApiProvider;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::FieldCondition;
use qdrant_client::qdrant::Filter;
use qdrant_client::qdrant::Match;
use qdrant_client::qdrant::Range;

pub struct QueryVectorDbHandler {
    config: VectorDbConfig,
}

impl QueryVectorDbHandler {
    pub fn new(config: VectorDbConfig) -> Self {
        Self { config }
    }
}

#[derive(Deserialize)]
struct QueryVectorDbArgs {
    query: String,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    doc_type: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    min_likes: Option<i64>,
    #[serde(default)]
    sentiment: Option<String>,
}

fn default_limit() -> usize {
    10
}

#[derive(Serialize)]
struct VectorSearchResult {
    id: String,
    score: f32,
    text: String,
    platform: Option<String>,
    doc_type: Option<String>,
    likes: Option<i64>,
    comments: Option<i64>,
    url: Option<String>,
    sentiment: Option<String>,
}

#[derive(Serialize)]
struct OpenAIEmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl ToolHandler for QueryVectorDbHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "query_vector_db handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: QueryVectorDbArgs = parse_arguments(&arguments)?;
        let codex_config = invocation.turn.client.config();
        let provider = super::openai_provider_for_tools(&codex_config)?;
        let api_provider = super::openai_api_provider(&provider)?;
        let api_key = super::resolve_openai_api_key(invocation.turn.as_ref(), &provider).await?;
        let client = build_reqwest_client();

        match query_qdrant(&args, &self.config, &api_provider, &api_key, &client).await {
            Ok(results) => {
                let json_results =
                    serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string());
                let count = results.len();

                Ok(ToolOutput::Function {
                    content: format!(
                        "Found {count} results from vector database:\n\n{json_results}"
                    ),
                    content_items: None,
                    success: Some(true),
                })
            }
            Err(e) => Err(FunctionCallError::RespondToModel(format!(
                "Failed to query vector database: {e}"
            ))),
        }
    }
}

async fn query_qdrant(
    args: &QueryVectorDbArgs,
    config: &VectorDbConfig,
    api_provider: &ApiProvider,
    api_key: &str,
    client: &Client,
) -> Result<Vec<VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
    let qdrant_client = Qdrant::from_url(&config.url).build()?;
    let collection_name = config.collection.as_str();

    let query_vector = generate_embedding(
        &args.query,
        api_provider,
        api_key,
        client,
        &config.embedding_model,
    )
    .await?;

    let mut conditions = Vec::new();

    if let Some(ref platform) = args.platform {
        conditions.push(
            FieldCondition {
                key: "platform".to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                        platform.clone(),
                    )),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
                is_empty: None,
                is_null: None,
            }
            .into(),
        );
    }

    if let Some(ref doc_type) = args.doc_type {
        conditions.push(
            FieldCondition {
                key: "doc_type".to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                        doc_type.clone(),
                    )),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
                is_empty: None,
                is_null: None,
            }
            .into(),
        );
    }

    if let Some(ref sentiment) = args.sentiment {
        conditions.push(
            FieldCondition {
                key: "sentiment".to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                        sentiment.clone(),
                    )),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
                is_empty: None,
                is_null: None,
            }
            .into(),
        );
    }

    if let Some(min_likes) = args.min_likes {
        conditions.push(
            FieldCondition {
                key: "likes".to_string(),
                r#match: None,
                range: Some(Range {
                    lt: None,
                    gt: None,
                    gte: Some(min_likes as f64),
                    lte: None,
                }),
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
                is_empty: None,
                is_null: None,
            }
            .into(),
        );
    }

    let query_filter = if !conditions.is_empty() {
        Some(Filter {
            must: conditions,
            ..Default::default()
        })
    } else {
        None
    };

    use qdrant_client::qdrant::SearchPointsBuilder;

    let mut search_builder =
        SearchPointsBuilder::new(collection_name, query_vector, args.limit as u64);

    if let Some(filter) = query_filter {
        search_builder = search_builder.filter(filter);
    }

    let search_request = search_builder.with_payload(true).build();
    let search_result = qdrant_client.search_points(search_request).await?;

    let results: Vec<VectorSearchResult> = search_result
        .result
        .into_iter()
        .map(|point| {
            let payload = point.payload;
            VectorSearchResult {
                id: point.id.map(|id| format!("{id:?}")).unwrap_or_default(),
                score: point.score,
                text: payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map_or("", |v| v)
                    .to_string(),
                platform: payload
                    .get("platform")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                doc_type: payload
                    .get("doc_type")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                likes: payload
                    .get("likes")
                    .and_then(qdrant_client::qdrant::Value::as_integer),
                comments: payload
                    .get("comments")
                    .and_then(qdrant_client::qdrant::Value::as_integer),
                url: payload
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                sentiment: payload
                    .get("sentiment")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
            }
        })
        .collect();

    Ok(results)
}

async fn generate_embedding(
    text: &str,
    api_provider: &ApiProvider,
    api_key: &str,
    client: &Client,
    embedding_model: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let request = OpenAIEmbeddingRequest {
        model: embedding_model.to_string(),
        input: text.to_string(),
    };

    let response = client
        .post(api_provider.url_for_path("embeddings"))
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

    let embedding_response: OpenAIEmbeddingResponse = response.json().await?;

    embedding_response
        .data
        .into_iter()
        .next()
        .map(|data| data.embedding)
        .ok_or_else(|| "No embedding returned from OpenAI".into())
}
