use reqwest::header;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

// ── Config types ────────────────────────────────────────────────────

/// A globally-registered LLM provider (external entity).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub base_url: String,
    #[serde(default)]
    pub default_model: String,
}

/// Known provider types. `Custom` is any OpenAI-compatible endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Groq,
    Ollama,
    OpenRouter,
    /// OpenAI-compatible (vLLM, Together, etc.)
    Custom,
    /// Local LLM via llama-helper sidecar — not an HTTP endpoint.
    Local,
}

// ── URL / auth helpers ──────────────────────────────────────────────

impl ProviderType {
    /// Returns the chat completions endpoint URL for this provider type.
    pub fn chat_completions_url(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        match self {
            ProviderType::Anthropic => format!("{}/messages", base),
            _ => format!("{}/chat/completions", base),
        }
    }

    /// Returns the models-listing endpoint URL for this provider type.
    pub fn models_url(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        match self {
            ProviderType::Ollama => format!("{}/api/tags", base),
            _ => format!("{}/models", base),
        }
    }

    /// Returns the auth header name (`Authorization` or `x-api-key`).
    pub fn auth_header_name(&self) -> &'static str {
        match self {
            ProviderType::Anthropic => "x-api-key",
            _ => "Authorization",
        }
    }

    /// Returns the auth header value for a given API key.
    pub fn auth_header_value(&self, api_key: &str) -> String {
        match self {
            ProviderType::Anthropic => api_key.to_string(),
            _ => format!("Bearer {}", api_key),
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ProviderType::OpenAI => "OpenAI",
            ProviderType::Anthropic => "Anthropic",
            ProviderType::Groq => "Groq",
            ProviderType::Ollama => "Ollama",
            ProviderType::OpenRouter => "OpenRouter",
            ProviderType::Custom => "Custom (OpenAI-compatible)",
            ProviderType::Local => "Local",
        }
    }
}

// ── Model listing ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

// Cache entry for model lists
struct ModelsCacheEntry {
    models: Vec<ProviderModel>,
    fetched_at: Instant,
}

static MODELS_CACHE: RwLock<Option<ModelsCacheEntry>> = RwLock::new(None);
const CACHE_TTL_SECS: u64 = 300;

/// Fetch available models for a provider by calling its models endpoint.
///
/// Results are cached for 5 minutes. Falls back to a built-in list when
/// the API cannot be reached.
pub async fn get_provider_models(
    provider: &ProviderConfig,
    api_key: Option<&str>,
) -> Result<Vec<ProviderModel>, String> {
    // Check cache
    {
        let cache = MODELS_CACHE.read().map_err(|e| e.to_string())?;
        if let Some(entry) = cache.as_ref() {
            if entry.fetched_at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                return Ok(entry.models.clone());
            }
        }
    }

    let url = provider.provider_type.models_url(&provider.base_url);

    let client = reqwest::Client::new();
    let mut req = client.get(&url).timeout(Duration::from_secs(5));

    // Add auth header if api key is provided
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        let header_name = provider.provider_type.auth_header_name();
        let header_val = provider.provider_type.auth_header_value(key);
        if header_name == "Authorization" {
            req = req.header(header::AUTHORIZATION, &header_val);
        } else {
            req = req.header(header_name, &header_val);
        }
    }

    let response = match req.send().await {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            let status = resp.status();
            log::warn!("Models endpoint returned {}; using fallback", status);
            return Ok(fallback_models(provider));
        }
        Err(e) => {
            log::warn!("Failed to fetch models from {}: {}; using fallback", url, e);
            return Ok(fallback_models(provider));
        }
    };

    let models = match provider.provider_type {
        ProviderType::Ollama => parse_ollama_models(response).await,
        ProviderType::Anthropic => parse_anthropic_models(response).await,
        _ => parse_openai_compat_models(response).await,
    };

    // Cache and return
    if let Ok(mut cache) = MODELS_CACHE.write() {
        *cache = Some(ModelsCacheEntry {
            models: models.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(models)
}

async fn parse_openai_compat_models(response: reqwest::Response) -> Vec<ProviderModel> {
    #[derive(Deserialize)]
    struct ApiModel {
        id: String,
    }
    #[derive(Deserialize)]
    struct ApiResponse {
        data: Vec<ApiModel>,
    }

    match response.json::<ApiResponse>().await {
        Ok(body) => body
            .data
            .into_iter()
            .map(|m| ProviderModel {
                id: m.id.clone(),
                name: m.id,
            })
            .collect(),
        Err(e) => {
            log::warn!("Failed to parse models response: {}", e);
            vec![]
        }
    }
}

async fn parse_ollama_models(response: reqwest::Response) -> Vec<ProviderModel> {
    #[derive(Deserialize)]
    struct OllamaModel {
        name: String,
    }
    #[derive(Deserialize)]
    struct OllamaResponse {
        models: Vec<OllamaModel>,
    }

    match response.json::<OllamaResponse>().await {
        Ok(body) => body
            .models
            .into_iter()
            .map(|m| ProviderModel {
                id: m.name.clone(),
                name: m.name,
            })
            .collect(),
        Err(e) => {
            log::warn!("Failed to parse Ollama models response: {}", e);
            vec![]
        }
    }
}

async fn parse_anthropic_models(response: reqwest::Response) -> Vec<ProviderModel> {
    #[derive(Deserialize)]
    struct AnthropicModel {
        id: String,
        #[allow(dead_code)]
        display_name: Option<String>,
    }
    #[derive(Deserialize)]
    struct AnthropicResponse {
        data: Vec<AnthropicModel>,
    }

    match response.json::<AnthropicResponse>().await {
        Ok(body) => body
            .data
            .into_iter()
            .map(|m| ProviderModel {
                id: m.id.clone(),
                name: m.display_name.unwrap_or_else(|| m.id.clone()),
            })
            .collect(),
        Err(e) => {
            log::warn!("Failed to parse Anthropic models response: {}", e);
            vec![]
        }
    }
}

/// Return a hardcoded fallback list when the API is unreachable.
fn fallback_models(provider: &ProviderConfig) -> Vec<ProviderModel> {
    let ids: &[&str] = match provider.provider_type {
        ProviderType::OpenAI => &[
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "gpt-4",
            "gpt-3.5-turbo",
            "o1",
            "o1-mini",
            "o3",
            "o3-mini",
        ],
        ProviderType::Anthropic => &[
            "claude-sonnet-4-5-20250929",
            "claude-haiku-4-5-20251001",
            "claude-opus-4-5-20251101",
        ],
        ProviderType::Groq => &[
            "llama-3.3-70b-versatile",
            "llama-3.1-70b-versatile",
            "mixtral-8x7b-32768",
            "gemma2-9b-it",
        ],
        ProviderType::Ollama => &[],
        ProviderType::OpenRouter => &[],
        ProviderType::Custom => &[&provider.default_model],
        ProviderType::Local => &[],
    };
    ids.iter()
        .filter(|id| !id.is_empty())
        .map(|id| ProviderModel {
            id: id.to_string(),
            name: id.to_string(),
        })
        .collect()
}
