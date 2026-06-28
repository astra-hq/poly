use crate::providers::ProviderType;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::info;

const REQUEST_TIMEOUT_DURATION: Duration = Duration::from_secs(300);

// Generic structure for OpenAI-compatible API chat messages
#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Generic structure for OpenAI-compatible API chat requests
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

// Generic structure for OpenAI-compatible API chat responses
#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub message: MessageContent,
}

#[derive(Deserialize, Debug)]
pub struct MessageContent {
    pub content: String,
}

// Claude-specific request structure
#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<ChatMessage>,
}

// Claude-specific response structure
#[derive(Deserialize, Debug)]
pub struct ClaudeChatResponse {
    pub content: Vec<ClaudeChatContent>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeChatContent {
    pub text: String,
}

/// Generates a summary using the specified LLM provider.
///
/// Uses `ProviderType` from the unified provider system instead of
/// the legacy per-provider enum.
///
/// # Arguments
/// * `client` - Reqwest HTTP client (reused for performance)
/// * `provider_type` - The LLM provider type (OpenAI, Anthropic, Groq, Ollama, etc.)
/// * `base_url` - The base URL for the provider API endpoint
/// * `model_name` - The specific model to use
/// * `api_key` - API key (not needed for Ollama and BuiltInAI)
/// * `system_prompt` - System instructions for the LLM
/// * `user_prompt` - User query/content to process
/// * `is_builtin` - Whether to use the local built-in AI sidecar instead of HTTP
/// * `max_tokens` - Optional max tokens override
/// * `temperature` - Optional temperature override
/// * `top_p` - Optional top_p override
/// * `app_data_dir` - App data directory (required for BuiltInAI)
/// * `cancellation_token` - Optional token to cancel the request
pub async fn generate_summary(
    client: &Client,
    provider_type: &ProviderType,
    base_url: &str,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    is_builtin: bool,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    app_data_dir: Option<&PathBuf>,
    cancellation_token: Option<&CancellationToken>,
) -> Result<String, String> {
    // Check if cancelled before starting
    if let Some(token) = cancellation_token {
        if token.is_cancelled() {
            return Err("Summary generation was cancelled".to_string());
        }
    }

    // Handle BuiltInAI provider separately (uses local sidecar, no HTTP API)
    if is_builtin {
        let app_data_dir = app_data_dir
            .ok_or_else(|| "app_data_dir is required for BuiltInAI provider".to_string())?;
        return crate::summary::summary_engine::generate_with_builtin(
            app_data_dir,
            model_name,
            system_prompt,
            user_prompt,
            cancellation_token,
        )
        .await
        .map_err(|e| e.to_string());
    }

    /// Internal enum for the two API formats we can try.
    #[derive(Debug)]
    enum RequestFormat {
        OpenAi,
        Anthropic,
    }

    let try_format = async |format: &RequestFormat| -> Result<reqwest::Response, String> {
        let api_url = provider_type.chat_completions_url(base_url);

        let mut headers = header::HeaderMap::new();
        match format {
            RequestFormat::Anthropic => {
                headers.insert(
                    "x-api-key",
                    api_key
                        .parse()
                        .map_err(|_| "Invalid API key format".to_string())?,
                );
                headers.insert(
                    "anthropic-version",
                    "2023-06-01"
                        .parse()
                        .map_err(|_| "Invalid anthropic version".to_string())?,
                );
            }
            RequestFormat::OpenAi => {
                headers.insert(
                    header::AUTHORIZATION,
                    format!("Bearer {}", api_key)
                        .parse()
                        .map_err(|_| "Invalid authorization header".to_string())?,
                );
            }
        }
        headers.insert(
            header::CONTENT_TYPE,
            "application/json"
                .parse()
                .map_err(|_| "Invalid content type".to_string())?,
        );

        let request_body = match format {
            RequestFormat::Anthropic => {
                serde_json::json!(ClaudeRequest {
                    system: system_prompt.to_string(),
                    model: model_name.to_string(),
                    max_tokens: 2048,
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: user_prompt.to_string(),
                    }]
                })
            }
            RequestFormat::OpenAi => {
                serde_json::json!(ChatRequest {
                    model: model_name.to_string(),
                    messages: vec![
                        ChatMessage {
                            role: "system".to_string(),
                            content: system_prompt.to_string(),
                        },
                        ChatMessage {
                            role: "user".to_string(),
                            content: user_prompt.to_string(),
                        }
                    ],
                    max_tokens,
                    temperature,
                    top_p,
                })
            }
        };

        let format_label = match format {
            RequestFormat::OpenAi => "OpenAI",
            RequestFormat::Anthropic => "Anthropic",
        };

        info!(
            "🐞 LLM Request ({} format) to {}: model={}",
            format_label,
            provider_type.label(),
            model_name
        );

        let request_future = client
            .post(&api_url)
            .headers(headers)
            .json(&request_body)
            .timeout(REQUEST_TIMEOUT_DURATION)
            .send();

        if let Some(token) = cancellation_token {
            tokio::select! {
                result = request_future => {
                    result.map_err(|e| {
                        if e.is_timeout() {
                            format!("LLM request timed out after 60 seconds")
                        } else {
                            format!("Failed to send request to LLM: {}", e)
                        }
                    })
                }
                _ = token.cancelled() => {
                    Err("Summary generation was cancelled".to_string())
                }
            }
        } else {
            request_future.await.map_err(|e| {
                if e.is_timeout() {
                    format!("LLM request timed out after 60 seconds")
                } else {
                    format!("Failed to send request to LLM: {}", e)
                }
            })
        }
    };

    let parse_response = async |response: reqwest::Response, format: &RequestFormat| -> Result<String, String> {
        if !response.status().is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("LLM API request failed: {}", error_body));
        }

        match format {
            RequestFormat::Anthropic => {
                let chat_response = response
                    .json::<ClaudeChatResponse>()
                    .await
                    .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

                info!("🐞 LLM Response received from Claude");

                let content = chat_response
                    .content
                    .get(0)
                    .ok_or("No content in LLM response")?
                    .text
                    .trim();
                Ok(content.to_string())
            }
            RequestFormat::OpenAi => {
                let chat_response = response
                    .json::<ChatResponse>()
                    .await
                    .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

                let provider_label = provider_type.label();
                info!("🐞 LLM Response received from {}", provider_label);

                let content = chat_response
                    .choices
                    .get(0)
                    .ok_or("No content in LLM response")?
                    .message
                    .content
                    .trim();
                Ok(content.to_string())
            }
        }
    };

    // For Custom providers, try OpenAI format first, then Anthropic as fallback.
    // Known providers only try their own format.
    let formats: &[RequestFormat] = if *provider_type == ProviderType::Custom {
        &[RequestFormat::OpenAi, RequestFormat::Anthropic]
    } else if *provider_type == ProviderType::Anthropic {
        &[RequestFormat::Anthropic]
    } else {
        &[RequestFormat::OpenAi]
    };

    let mut last_error = String::new();
    for format in formats {
        match try_format(format).await {
            Ok(response) => match parse_response(response, format).await {
                Ok(content) => return Ok(content),
                Err(e) => {
                    // Parsing failed — if we have more formats to try, log and continue
                    last_error = e;
                    info!("LLM response parsing failed with {:?} format, will retry with next format if available: {}", format, last_error);
                }
            },
            Err(e) => {
                last_error = e;
                info!("LLM request failed with {:?} format, will retry with next format if available: {}", format, last_error);
            }
        }
    }

    Err(last_error)
}

/// Legacy wrapper for backward compatibility during migration.
/// Uses the new `generate_summary` underneath.
#[deprecated(note = "Use generate_summary with ProviderType instead")]
pub async fn generate_summary_legacy(
    client: &Client,
    provider: &super::LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    custom_openai_endpoint: Option<&str>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    app_data_dir: Option<&PathBuf>,
    cancellation_token: Option<&CancellationToken>,
) -> Result<String, String> {
    let (provider_type, base_url, is_builtin) = legacy_to_provider_params(
        provider,
        ollama_endpoint,
        custom_openai_endpoint,
    );
    let (resolved_max_tokens, resolved_temperature, resolved_top_p) = if provider == &super::LLMProvider::CustomOpenAI {
        (max_tokens, temperature, top_p)
    } else {
        (None, None, None)
    };
    generate_summary(
        client,
        &provider_type,
        &base_url,
        model_name,
        api_key,
        system_prompt,
        user_prompt,
        is_builtin,
        resolved_max_tokens,
        resolved_temperature,
        resolved_top_p,
        app_data_dir,
        cancellation_token,
    ).await
}

fn legacy_to_provider_params(
    provider: &super::LLMProvider,
    ollama_endpoint: Option<&str>,
    custom_openai_endpoint: Option<&str>,
) -> (ProviderType, String, bool) {
    match provider {
        super::LLMProvider::OpenAI => (
            ProviderType::OpenAI,
            "https://api.openai.com/v1".to_string(),
            false,
        ),
        super::LLMProvider::Claude => (
            ProviderType::Anthropic,
            "https://api.anthropic.com".to_string(),
            false,
        ),
        super::LLMProvider::Groq => (
            ProviderType::Groq,
            "https://api.groq.com/openai/v1".to_string(),
            false,
        ),
        super::LLMProvider::Ollama => (
            ProviderType::Ollama,
            ollama_endpoint
                .map(|s| s.to_string())
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            false,
        ),
        super::LLMProvider::OpenRouter => (
            ProviderType::OpenRouter,
            "https://openrouter.ai/api/v1".to_string(),
            false,
        ),
        super::LLMProvider::BuiltInAI => (
            ProviderType::Ollama,
            String::new(),
            true,
        ),
        super::LLMProvider::CustomOpenAI => (
            ProviderType::Custom,
            custom_openai_endpoint
                .unwrap_or("")
                .to_string(),
            false,
        ),
    }
}

/// Legacy LLM provider enum — deprecated. Use `ProviderType` instead.
#[derive(Debug, Clone, PartialEq)]
pub enum LLMProvider {
    OpenAI,
    Claude,
    Groq,
    Ollama,
    OpenRouter,
    BuiltInAI,
    CustomOpenAI,
}

impl LLMProvider {
    /// Parse provider from string (case-insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "groq" => Ok(Self::Groq),
            "ollama" => Ok(Self::Ollama),
            "openrouter" => Ok(Self::OpenRouter),
            "builtin-ai" | "local-llama" | "localllama" => Ok(Self::BuiltInAI),
            "custom-openai" => Ok(Self::CustomOpenAI),
            _ => Err(format!("Unsupported LLM provider: {}", s)),
        }
    }
}
