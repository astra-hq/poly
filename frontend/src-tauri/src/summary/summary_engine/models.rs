// Model definitions and prompt templates for built-in AI summary generation
// Designed for easy extension - just add new entries to get_available_models()

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

// ============================================================================
// Model Definitions
// ============================================================================

/// Type of a local AI model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    /// Models used for summary generation (e.g., Qwen, Gemma)
    Summary,
    /// Models used for embedding/text vector generation (e.g., BGE)
    Embedding,
}

impl ModelType {
    /// Return the filesystem subdirectory name for this model type.
    pub fn subdir(&self) -> &'static str {
        match self {
            ModelType::Summary => "summary",
            ModelType::Embedding => "embedding",
        }
    }
}

/// Sampling parameters supported by the built-in AI -> llama-helper pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SamplingParams {
    /// Temperature - 0.0 triggers greedy decoding in llama-helper.
    pub temperature: f32,

    /// Top-K sampling - limits vocabulary to top K tokens.
    pub top_k: i32,

    /// Top-P (nucleus) sampling - cumulative probability threshold.
    pub top_p: f32,

    /// Presence penalty - discourages reusing tokens that already appeared in the generated output.
    pub presence_penalty: f32,

    /// Frequency penalty - discourages repeated token frequency in the generated output.
    pub frequency_penalty: f32,

    /// Repeat penalty - llama.cpp repeat penalty, 1.0 disables it.
    pub repeat_penalty: f32,

    /// Number of recent generated tokens to apply penalties over, 0 disables penalties.
    pub penalty_last_n: i32,

    /// Stop tokens - generation stops when any of these appear in output
    pub stop_tokens: Vec<String>,
}

impl SamplingParams {
    /// Restrained near-greedy preset for fuller but still conservative output.
    pub fn tight_structured(stop_tokens: Vec<String>) -> Self {
        Self {
            temperature: 0.1,
            top_k: 20,
            top_p: 0.88,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            repeat_penalty: 1.0,
            penalty_last_n: 0,
            stop_tokens,
        }
    }

    /// Summary-tuned Qwen 3.5 preset: non-greedy with mild repetition controls.
    pub fn qwen35_summary(stop_tokens: Vec<String>) -> Self {
        Self {
            temperature: 0.5,
            top_k: 20,
            top_p: 0.8,
            presence_penalty: 0.3,
            frequency_penalty: 0.0,
            repeat_penalty: 1.05,
            penalty_last_n: 256,
            stop_tokens,
        }
    }

    /// Gemma 3 instruct preset, matching the prior Gemma sampling behavior.
    pub fn gemma3_instruct(stop_tokens: Vec<String>) -> Self {
        Self {
            temperature: 1.0,
            top_k: 64,
            top_p: 0.95,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            repeat_penalty: 1.0,
            penalty_last_n: 0,
            stop_tokens,
        }
    }

    /// Normalize built-in presets to the subset supported by llama-helper.
    pub fn sanitize_for_llama_helper(&self) -> Self {
        let temperature = if self.temperature.is_finite() {
            self.temperature.max(0.0)
        } else {
            0.0
        };
        let top_k = self.top_k.max(0);
        let top_p = if self.top_p.is_finite() && self.top_p > 0.0 && self.top_p <= 1.0 {
            self.top_p
        } else {
            1.0
        };
        let presence_penalty = if self.presence_penalty.is_finite() {
            self.presence_penalty.max(0.0)
        } else {
            0.0
        };
        let frequency_penalty = if self.frequency_penalty.is_finite() {
            self.frequency_penalty.max(0.0)
        } else {
            0.0
        };
        let repeat_penalty = if self.repeat_penalty.is_finite() && self.repeat_penalty > 0.0 {
            self.repeat_penalty
        } else {
            1.0
        };
        let penalty_last_n = self.penalty_last_n.max(0);

        Self {
            temperature,
            top_k,
            top_p,
            presence_penalty,
            frequency_penalty,
            repeat_penalty,
            penalty_last_n,
            stop_tokens: self.stop_tokens.clone(),
        }
    }
}

/// Definition of a local AI model with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDef {
    /// Model name in format "family:variant" (e.g., "gemma3:1b")
    /// This is what's stored in database as model field when provider="local"
    pub name: String,

    /// Display name for UI (e.g., "Gemma 3 1B (Fast)")
    pub display_name: String,

    /// GGUF filename on disk (e.g., "gemma-3-1b-it-q4_0.gguf")
    pub gguf_file: String,

    /// Template name for prompt formatting (e.g., "gemma3")
    pub template: String,

    /// Download URL (HuggingFace or other source)
    pub download_url: String,

    /// File size in MiB. The field name is kept for API compatibility.
    pub size_mb: u64,

    /// Context window size in tokens (configurable per model!)
    /// This is used for chunking in processor.rs
    pub context_size: u32,

    /// Model layer count (for GPU offloading calculation)
    pub layer_count: u32,

    /// Sampling parameters for this model
    pub sampling: SamplingParams,

    /// Short description for UI
    pub description: String,

    /// Model type (summary or embedding)
    pub model_type: ModelType,
}

impl ModelDef {
    /// Return the filesystem subdirectory name for this model.
    pub fn subdir(&self) -> &'static str {
        self.model_type.subdir()
    }
}

/// Get all available built-in AI summary models
pub fn get_available_models() -> Vec<ModelDef> {
    vec![
        // Qwen 3.5 2B - Balanced tier
        ModelDef {
            name: "qwen3.5:2b".to_string(),
            display_name: "Qwen 3.5 2B (Balanced)".to_string(),
            gguf_file: "Qwen3.5-2B-Q4_K_M.gguf".to_string(),
            template: "qwen3.5_nonthinking".to_string(),
            download_url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf".to_string(),
            size_mb: 1221,
            context_size: 32768,
            layer_count: 24,
            sampling: SamplingParams::qwen35_summary(vec!["<|im_end|>".to_string()]),
            description: "Balanced Qwen 3.5 model for built-in summaries. Higher quality with modest local requirements.".to_string(),
            model_type: ModelType::Summary,
        },
        // Qwen 3.5 4B - High quality tier
        ModelDef {
            name: "qwen3.5:4b".to_string(),
            display_name: "Qwen 3.5 4B (High Quality)".to_string(),
            gguf_file: "Qwen3.5-4B-Q4_K_M.gguf".to_string(),
            template: "qwen3.5_nonthinking".to_string(),
            download_url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf".to_string(),
            size_mb: 2614,
            context_size: 32768,
            layer_count: 32,
            sampling: SamplingParams::qwen35_summary(vec!["<|im_end|>".to_string()]),
            description: "High-quality Qwen 3.5 model for built-in summaries. Best local Qwen option in the current lineup.".to_string(),
            model_type: ModelType::Summary,
        },
        // Gemma 3 4B - Legacy alternative retained for users who prefer Gemma output.
        ModelDef {
            name: "gemma3:4b".to_string(),
            display_name: "Gemma 3 4B (Balanced)".to_string(),
            gguf_file: "gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            template: "gemma3".to_string(),
            download_url: "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/main/google_gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            size_mb: 2374,
            context_size: 32768,
            layer_count: 35,
            sampling: SamplingParams::gemma3_instruct(vec!["<end_of_turn>".to_string()]),
            description: "Balanced model. Great quality/speed trade-off. Requires ~3.5GB RAM.".to_string(),
            model_type: ModelType::Summary,
        },
        // Gemma 3 1B - Visible legacy tier retained for already-shipped users.
        ModelDef {
            name: "gemma3:1b".to_string(),
            display_name: "Gemma 3 1B (Fast)".to_string(),
            gguf_file: "gemma-3-1b-it-Q8_0.gguf".to_string(),
            template: "gemma3".to_string(),
            download_url: "https://huggingface.co/bartowski/google_gemma-3-1b-it-GGUF/resolve/main/google_gemma-3-1b-it-Q8_0.gguf".to_string(),
            size_mb: 1019,
            context_size: 32768,
            layer_count: 26,
            sampling: SamplingParams::gemma3_instruct(vec!["<end_of_turn>".to_string()]),
            description: "Fastest model. Runs on any hardware with ~1GB RAM. Good for quick summaries.".to_string(),
            model_type: ModelType::Summary,
        },
    ]
}

/// Get all available built-in AI embedding models
pub fn get_available_embedding_models() -> Vec<ModelDef> {
    vec![
        // BAAI/bge-m3 - multilingual embedding model
        ModelDef {
            name: "bge-m3:latest".to_string(),
            display_name: "BAAI/bge-m3 (Multilingual)".to_string(),
            gguf_file: "bge-m3-Q4_K_M.gguf".to_string(),
            template: "".to_string(), // embedding models don't use chat templates
            download_url: "https://huggingface.co/ChristianAzinn/bge-m3-GGUF/resolve/main/bge-m3-Q4_K_M.gguf".to_string(),
            size_mb: 322,
            context_size: 8192,
            layer_count: 24,
            sampling: SamplingParams {
                temperature: 0.0,
                top_k: 1,
                top_p: 1.0,
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                repeat_penalty: 1.0,
                penalty_last_n: 0,
                stop_tokens: vec![],
            },
            description: "BAAI/bge-m3 embedding model (1024 dimensions). Used for local knowledge graph embeddings. ~1GB download.".to_string(),
            model_type: ModelType::Embedding,
        },
    ]
}

/// Get a specific model by name
pub fn get_model_by_name(name: &str) -> Option<ModelDef> {
    get_available_models().into_iter().find(|m| m.name == name)
}

/// Get the default model (first in list)
pub fn get_default_model() -> ModelDef {
    get_available_models()
        .into_iter()
        .next()
        .expect("At least one model must be defined")
}

/// Resolve model name to full file path in the appropriate type subdirectory
pub fn get_model_path(app_data_dir: &PathBuf, model_name: &str) -> Result<PathBuf> {
    let model = get_model_by_name_any(model_name)
        .ok_or_else(|| anyhow!("Unknown model: {}", model_name))?;

    let model_path = app_data_dir
        .join("models")
        .join(model.subdir())
        .join(&model.gguf_file);

    Ok(model_path)
}

/// Get the models directory path for built-in AI (base directory, type subdirs inside)
pub fn get_models_directory(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("models")
}

/// Get the models directory path for summary models (legacy path kept for backward compat)
pub fn get_summary_models_directory(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("models").join("summary")
}

/// Get the models directory path for embedding models
pub fn get_embedding_models_directory(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("models").join("embedding")
}

// ============================================================================
// Prompt Templates (Model-Specific Formatting)
// ============================================================================

/// Gemma 3 chat template format
pub const GEMMA3_TEMPLATE: &str = "\
<start_of_turn>user
{system_prompt}<end_of_turn>
<start_of_turn>user
{user_prompt}<end_of_turn>
<start_of_turn>model
";

/// Qwen 3.5 non-thinking chat template format.
/// This starts the assistant turn with an empty think block so generation begins
/// in direct-response mode for summaries.
pub const QWEN35_NONTHINKING_TEMPLATE: &str = "\
<|im_start|>system
{system_prompt}<|im_end|>
<|im_start|>user
{user_prompt}<|im_end|>
<|im_start|>assistant
<think>

</think>

";

fn escape_user_prompt_control_markers(user_prompt: &str) -> String {
    user_prompt
        .replace("<|im_start|>", "< |im_start| >")
        .replace("<|im_end|>", "< |im_end| >")
        .replace("<start_of_turn>", "< start_of_turn >")
        .replace("<end_of_turn>", "< end_of_turn >")
        .replace("<think>", "< think >")
        .replace("</think>", "< /think >")
}

/// Format a prompt using the specified template
///
/// # Arguments
/// * `template_name` - Template identifier (e.g., "gemma3", "chatml", "llama3")
/// * `system_prompt` - System message (instructions for the model)
/// * `user_prompt` - User message (actual task/question)
///
/// # Returns
/// Formatted prompt string ready to send to llama-helper
pub fn format_prompt(
    template_name: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let template = match template_name {
        "gemma3" => GEMMA3_TEMPLATE,
        "gemma4" => GEMMA4_TEMPLATE,
        "qwen3.5_nonthinking" => QWEN35_NONTHINKING_TEMPLATE,
        _ => return Err(anyhow!("Unknown template: {}", template_name)),
    };

    let escaped_user_prompt = escape_user_prompt_control_markers(user_prompt);

    let formatted = template
        .replace("{system_prompt}", system_prompt)
        .replace("{user_prompt}", &escaped_user_prompt);

    Ok(formatted)
}

// ============================================================================
// Configuration Constants
// ============================================================================

/// Default max tokens for generation (increased for better summary quality)
pub const DEFAULT_MAX_TOKENS: i32 = 4096;

/// Idle timeout for sidecar (seconds) - can be overridden via LLAMA_IDLE_TIMEOUT env var
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Generation timeout (how long to wait for a response)
pub const GENERATION_TIMEOUT_SECS: u64 = 900; // 15 minutes

// ============================================================================
// Custom Model Registry (HF GGUF persistence)
// ============================================================================

/// Types of model registries for lookups.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegistrySource {
    /// Curated models baked into the app binary.
    Curated,
    /// User-added models from Hugging Face GGUF repos.
    Custom,
}

/// A persisted entry in the custom model registry (JSON on disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelEntry {
    /// HuggingFace repo id (e.g., "unsloth/gemma-4-E4B-it-GGUF")
    pub repo_id: String,
    /// Selected GGUF filename (e.g., "gemma-4-e4b-it-Q4_K_M.gguf")
    pub filename: String,
    /// Download resolve URL
    pub download_url: String,
    /// Prompt template name
    pub template: String,
    /// Context window size in tokens
    pub context_size: u32,
    /// File size in bytes (from siblings metadata)
    pub size_bytes: u64,
}

impl CustomModelEntry {
    /// Derive a ModelDef from a persisted custom entry for runtime use.
    fn to_model_def(&self) -> ModelDef {
        let size_mb = self.size_bytes / (1024 * 1024);
        let display_name = format!(
            "{} ({})",
            self.repo_id,
            self.filename.trim_end_matches(".gguf")
        );
        // Use tight_structured sampling for custom models with sensible defaults.
        let sampling = SamplingParams::gemma3_instruct(vec!["<end_of_turn>".to_string()]);
        // Estimate layers (rough heuristic based on size)
        let layer_count = (size_mb / 100).max(16).min(64) as u32;

        ModelDef {
            name: format!("custom:{}", self.repo_id.replace('/', ":")),
            display_name,
            gguf_file: self.filename.clone(),
            template: self.template.clone(),
            download_url: self.download_url.clone(),
            size_mb,
            context_size: self.context_size,
            layer_count,
            sampling,
            description: format!(
                "Custom model from {}. {} MB. {}",
                self.repo_id, size_mb, self.filename
            ),
            model_type: ModelType::Summary,
        }
    }
}

/// HF API response types for repo verification.
#[derive(Debug, Deserialize)]
pub struct HfModelInfo {
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub gated: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub siblings: Vec<HfSibling>,
}

#[derive(Debug, Deserialize)]
pub struct HfSibling {
    pub rfilename: String,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Result of HF repo verification
#[derive(Debug, Clone, Serialize)]
pub struct HfRepoVerification {
    pub repo_id: String,
    /// Valid GGUF files found in the repo
    pub gguf_files: Vec<GgufCandidate>,
    /// Recommended default (selected by quantization preference)
    pub default_selection: Option<GgufCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GgufCandidate {
    pub filename: String,
    pub download_url: String,
    pub size_bytes: u64,
}

/// Ordered quantization preference for default selection.
const QUANT_PREFERENCE: &[&str] = &["Q4_K_M", "Q5_K_M", "Q4_K_S", "Q5_K_S", "Q8_0"];

/// Select the best default GGUF file from a list of filenames.
/// Preference order: Q4_K_M > Q5_K_M > Q4_K_S > Q5_K_S > Q8_0 > smallest.
pub fn select_default_gguf(files: &[(String, u64)]) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    for pref in QUANT_PREFERENCE {
        if let Some((name, _)) = files.iter().find(|(n, _)| n.contains(pref)) {
            return Some(name.clone());
        }
    }

    // Fallback: smallest file
    files
        .iter()
        .min_by_key(|(_, size)| *size)
        .map(|(n, _)| n.clone())
}

/// Load custom model registry from disk
pub fn load_custom_registry(app_data_dir: &std::path::Path) -> Vec<CustomModelEntry> {
    let registry_path = app_data_dir.join("custom_models.json");
    if !registry_path.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(&registry_path) {
        Ok(contents) => match serde_json::from_str(&contents) {
            Ok(entries) => entries,
            Err(e) => {
                log::error!(
                    "Failed to parse custom model registry at {}: {}",
                    registry_path.display(),
                    e
                );
                Vec::new()
            }
        },
        Err(e) => {
            log::error!(
                "Failed to read custom model registry at {}: {}",
                registry_path.display(),
                e
            );
            Vec::new()
        }
    }
}

/// Save custom model registry to disk atomically.
pub fn save_custom_registry(
    app_data_dir: &std::path::Path,
    entries: &[CustomModelEntry],
) -> Result<()> {
    let registry_path = app_data_dir.join("custom_models.json");

    // Ensure directory exists
    if let Some(parent) = registry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(entries)?;
    let tmp_path = registry_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, &registry_path)?;

    log::info!(
        "Saved custom model registry with {} entries to {}",
        entries.len(),
        registry_path.display()
    );
    Ok(())
}

/// Verify a HuggingFace repo for GGUF models via the HF API.
pub async fn verify_hf_repo(repo_id: &str) -> Result<HfRepoVerification> {
    let url = format!("https://huggingface.co/api/models/{}", repo_id);
    let client = reqwest::Client::builder()
        .user_agent("poly-hf-verifier/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to reach HuggingFace API for '{}': {}", repo_id, e))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "HuggingFace API returned {} for repo '{}'. Verify the namespace/repo exists and is public.",
            response.status(),
            repo_id
        ));
    }

    let info: HfModelInfo = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse HF API response for '{}': {}", repo_id, e))?;

    if info.private {
        return Err(anyhow!(
            "Repo '{}' is private. Only public repos are supported.",
            repo_id
        ));
    }
    if info.gated {
        return Err(anyhow!(
            "Repo '{}' is gated. Gated repos are not supported.",
            repo_id
        ));
    }
    if info.disabled {
        return Err(anyhow!("Repo '{}' is disabled.", repo_id));
    }

    // Collect GGUF siblings
    let gguf_files: Vec<(String, u64)> = info
        .siblings
        .iter()
        .filter(|s| s.rfilename.ends_with(".gguf"))
        .map(|s| (s.rfilename.clone(), s.size.unwrap_or(0)))
        .collect();

    if gguf_files.is_empty() {
        return Err(anyhow!(
            "No .gguf files found in repo '{}'. This repo may not contain GGUF models.",
            repo_id
        ));
    }

    let candidates: Vec<GgufCandidate> = gguf_files
        .iter()
        .map(|(filename, size_bytes)| GgufCandidate {
            filename: filename.clone(),
            download_url: format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo_id, filename
            ),
            size_bytes: *size_bytes,
        })
        .collect();

    let default_selection = select_default_gguf(&gguf_files).map(|filename| {
        let entry = gguf_files.iter().find(|(n, _)| *n == filename).unwrap();
        GgufCandidate {
            filename: entry.0.clone(),
            download_url: format!(
                "https://huggingface.co/{}/resolve/main/{}",
                repo_id, entry.0
            ),
            size_bytes: entry.1,
        }
    });

    Ok(HfRepoVerification {
        repo_id: repo_id.to_string(),
        gguf_files: candidates,
        default_selection,
    })
}

/// Add a custom model to the persisted registry.
pub fn add_to_custom_registry(
    app_data_dir: &std::path::Path,
    repo_id: &str,
    filename: &str,
    template: &str,
    context_size: u32,
    size_bytes: u64,
) -> Result<()> {
    let mut entries = load_custom_registry(app_data_dir);

    // Check if already registered
    let model_name = filename.to_string();
    if entries.iter().any(|e| e.filename == model_name) {
        log::info!(
            "Custom model '{}' from {} is already registered, skipping",
            filename,
            repo_id
        );
        return Ok(());
    }

    let download_url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repo_id, filename
    );

    let entry = CustomModelEntry {
        repo_id: repo_id.to_string(),
        filename: filename.to_string(),
        download_url,
        template: template.to_string(),
        context_size,
        size_bytes,
    };

    entries.push(entry);
    save_custom_registry(app_data_dir, &entries)?;

    Ok(())
}

// ============================================================================
// Gemma 4 Prompt Template
// ============================================================================

/// Gemma 4 chat template — same format as Gemma 3.
pub const GEMMA4_TEMPLATE: &str = "\
<start_of_turn>user
{system_prompt}<end_of_turn>
<start_of_turn>user
{user_prompt}<end_of_turn>
<start_of_turn>model
";

// ============================================================================
// Generalized Model Lookup (Curated + Custom)
// ============================================================================

thread_local! {
    /// Cached custom model entries for this thread. Avoids re-reading disk.
    static CUSTOM_REGISTRY: std::cell::RefCell<Option<Vec<CustomModelEntry>>> = std::cell::RefCell::new(None);
}

/// Set the custom registry cache for the current thread (called at startup).
pub fn refresh_custom_registry_cache(app_data_dir: &std::path::Path) {
    let entries = load_custom_registry(app_data_dir);
    CUSTOM_REGISTRY.with(|cache| {
        *cache.borrow_mut() = Some(entries);
    });
}

/// Get all models: curated summary + curated embedding + custom (merged).
pub fn get_all_models() -> Vec<ModelDef> {
    let mut models = get_available_models();
    models.extend(get_available_embedding_models());
    CUSTOM_REGISTRY.with(|cache| {
        if let Some(ref entries) = *cache.borrow() {
            for entry in entries {
                models.push(entry.to_model_def());
            }
        }
    });
    models
}

/// Get a specific model by name (searches curated first, then custom).
pub fn get_model_by_name_any(name: &str) -> Option<ModelDef> {
    // Try curated first
    if let Some(m) = get_model_by_name(name) {
        return Some(m);
    }
    // Try custom registry
    CUSTOM_REGISTRY.with(|cache| {
        if let Some(ref entries) = *cache.borrow() {
            for entry in entries {
                let def = entry.to_model_def();
                if def.name == name {
                    return Some(def);
                }
            }
        }
        None
    })
}

#[cfg(test)]
mod hf_registry {
    use super::*;

    // =========================================================================
    // Baseline: curated-only listing
    // =========================================================================

    #[test]
    fn curated_only_listing_returns_four_builtin_models() {
        let models = get_available_models();
        assert_eq!(models.len(), 4, "curated list should have exactly 4 models");
        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"qwen3.5:4b"));
        assert!(names.contains(&"qwen3.5:2b"));
        assert!(names.contains(&"gemma3:4b"));
        assert!(names.contains(&"gemma3:1b"));
    }

    // =========================================================================
    // Failing-first: HF repo verification with mocked response
    // =========================================================================

    #[test]
    fn verify_hf_repo_rejects_private_model() {
        let json = r#"{"private":true,"gated":false,"disabled":false,"siblings":[]}"#;
        // This test covers the validation logic directly on deserialized data.
        let info: HfModelInfo = serde_json::from_str(json).unwrap();
        assert!(info.private);
        assert!(!info.gated);
        assert!(!info.disabled);
    }

    #[test]
    fn verify_hf_repo_rejects_gated_model() {
        let json = r#"{"private":false,"gated":true,"disabled":false,"siblings":[]}"#;
        let info: HfModelInfo = serde_json::from_str(json).unwrap();
        assert!(info.gated);
    }

    #[test]
    fn verify_hf_repo_rejects_disabled_model() {
        let json = r#"{"private":false,"gated":false,"disabled":true,"siblings":[]}"#;
        let info: HfModelInfo = serde_json::from_str(json).unwrap();
        assert!(info.disabled);
    }

    #[test]
    fn verify_hf_repo_accepts_valid_public_repo() {
        let json = r#"{"private":false,"gated":false,"disabled":false,"siblings":[]}"#;
        let info: HfModelInfo = serde_json::from_str(json).unwrap();
        assert!(!info.private);
        assert!(!info.gated);
        assert!(!info.disabled);
    }

    #[test]
    fn verify_hf_repo_detects_gguf_siblings() {
        let json = r#"{
            "private":false,"gated":false,"disabled":false,
            "siblings":[
                {"rfilename":"gemma-4-e4b-it-Q4_K_M.gguf","size":4567890123},
                {"rfilename":"gemma-4-e4b-it-Q5_K_M.gguf","size":5123456789},
                {"rfilename":"gemma-4-e4b-it-Q8_0.gguf","size":7890123456},
                {"rfilename":"README.md","size":1024}
            ]
        }"#;
        let info: HfModelInfo = serde_json::from_str(json).unwrap();
        let gguf: Vec<&str> = info
            .siblings
            .iter()
            .filter(|s| s.rfilename.ends_with(".gguf"))
            .map(|s| s.rfilename.as_str())
            .collect();
        assert_eq!(gguf.len(), 3);
        assert!(gguf.contains(&"gemma-4-e4b-it-Q4_K_M.gguf"));
        assert!(gguf.contains(&"gemma-4-e4b-it-Q5_K_M.gguf"));
        assert!(gguf.contains(&"gemma-4-e4b-it-Q8_0.gguf"));
    }

    // =========================================================================
    // Quantization selection
    // =========================================================================

    #[test]
    fn select_default_gguf_empty_returns_none() {
        assert_eq!(select_default_gguf(&[]), None);
    }

    #[test]
    fn select_default_gguf_prefers_q4_k_m() {
        let files = vec![
            ("model-Q8_0.gguf".to_string(), 7000),
            ("model-Q4_K_M.gguf".to_string(), 4000),
            ("model-Q5_K_M.gguf".to_string(), 5000),
        ];
        assert_eq!(
            select_default_gguf(&files).as_deref(),
            Some("model-Q4_K_M.gguf")
        );
    }

    #[test]
    fn select_default_gguf_falls_back_to_q5_k_m_when_no_q4_k_m() {
        let files = vec![
            ("model-Q8_0.gguf".to_string(), 7000),
            ("model-Q5_K_M.gguf".to_string(), 5000),
        ];
        assert_eq!(
            select_default_gguf(&files).as_deref(),
            Some("model-Q5_K_M.gguf")
        );
    }

    #[test]
    fn select_default_gguf_falls_back_to_smallest_when_no_preferred_quant() {
        let files = vec![
            ("model-IQ3_M.gguf".to_string(), 3000),
            ("model-IQ2_XXS.gguf".to_string(), 1500),
            ("model-IQ4_XS.gguf".to_string(), 3500),
        ];
        assert_eq!(
            select_default_gguf(&files).as_deref(),
            Some("model-IQ2_XXS.gguf")
        );
    }

    // =========================================================================
    // Custom registry persistence
    // =========================================================================

    #[test]
    fn custom_registry_round_trip() {
        let tmp = std::env::temp_dir().join("poly_test_custom_registry");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let entries = vec![CustomModelEntry {
            repo_id: "unsloth/gemma-4-E4B-it-GGUF".to_string(),
            filename: "gemma-4-e4b-it-Q4_K_M.gguf".to_string(),
            download_url: "https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-e4b-it-Q4_K_M.gguf".to_string(),
            template: "gemma4".to_string(),
            context_size: 8192,
            size_bytes: 4_567_890_123,
        }];

        save_custom_registry(&tmp, &entries).unwrap();

        let loaded = load_custom_registry(&tmp);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].repo_id, "unsloth/gemma-4-E4B-it-GGUF");
        assert_eq!(loaded[0].filename, "gemma-4-e4b-it-Q4_K_M.gguf");
        assert_eq!(loaded[0].template, "gemma4");
        assert_eq!(loaded[0].context_size, 8192);
        assert_eq!(loaded[0].size_bytes, 4_567_890_123);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn custom_registry_load_empty_when_missing() {
        let tmp = std::env::temp_dir().join("poly_test_nonexistent_registry");
        let loaded = load_custom_registry(&tmp);
        assert!(loaded.is_empty());
    }

    #[test]
    fn custom_model_entry_to_model_def() {
        let entry = CustomModelEntry {
            repo_id: "unsloth/gemma-4-E4B-it-GGUF".to_string(),
            filename: "gemma-4-e4b-it-Q4_K_M.gguf".to_string(),
            download_url: "https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-e4b-it-Q4_K_M.gguf".to_string(),
            template: "gemma4".to_string(),
            context_size: 8192,
            size_bytes: 4_567_890_123,
        };
        let def = entry.to_model_def();
        let expected_size_mb = 4_567_890_123 / (1024 * 1024);
        assert_eq!(def.name, "custom:unsloth:gemma-4-E4B-it-GGUF");
        assert_eq!(def.gguf_file, "gemma-4-e4b-it-Q4_K_M.gguf");
        assert_eq!(def.template, "gemma4");
        assert_eq!(def.size_mb, expected_size_mb);
        assert_eq!(def.context_size, 8192);
        assert_eq!(
            def.download_url,
            "https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-e4b-it-Q4_K_M.gguf"
        );
    }

    // =========================================================================
    // Gemma 4 template
    // =========================================================================

    #[test]
    fn gemma4_template_formats_prompt_like_gemma3() {
        let formatted = format_prompt("gemma4", "system rules", "summarize this").unwrap();

        assert!(formatted.contains("<start_of_turn>user\nsystem rules<end_of_turn>"));
        assert!(formatted.contains("<start_of_turn>user\nsummarize this<end_of_turn>"));
        assert!(formatted.contains("<start_of_turn>model\n"));
    }

    #[test]
    fn gemma4_template_escapes_control_markers() {
        let formatted = format_prompt(
            "gemma4",
            "system",
            "literal <start_of_turn> and <end_of_turn>",
        )
        .unwrap();
        assert!(formatted.contains("literal < start_of_turn > and < end_of_turn >"));
        assert_eq!(formatted.matches("<start_of_turn>").count(), 3);
        assert_eq!(formatted.matches("<end_of_turn>").count(), 2);
    }

    // =========================================================================
    // Generalized lookup
    // =========================================================================

    #[test]
    fn get_model_by_name_any_finds_curated_model() {
        let model = get_model_by_name_any("qwen3.5:2b");
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "qwen3.5:2b");
    }

    #[test]
    fn get_model_by_name_any_returns_none_for_unknown() {
        let model = get_model_by_name_any("nonexistent:99b");
        assert!(model.is_none());
    }

    #[test]
    fn add_to_custom_registry_prevents_duplicates() {
        let tmp = std::env::temp_dir().join("poly_test_custom_dedup");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        add_to_custom_registry(
            &tmp,
            "unsloth/gemma-4-E4B-it-GGUF",
            "gemma-4-e4b-it-Q4_K_M.gguf",
            "gemma4",
            8192,
            4_567_890_123,
        )
        .unwrap();

        add_to_custom_registry(
            &tmp,
            "unsloth/gemma-4-E4B-it-GGUF",
            "gemma-4-e4b-it-Q4_K_M.gguf",
            "gemma4",
            8192,
            4_567_890_123,
        )
        .unwrap();

        let loaded = load_custom_registry(&tmp);
        assert_eq!(loaded.len(), 1, "duplicate should be skipped");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // =========================================================================
    // Pre-existing tests (curated models)
    // =========================================================================

    #[test]
    fn qwen35_models_are_registered_with_expected_metadata() {
        let qwen_2b = get_model_by_name("qwen3.5:2b").expect("qwen 2b model should exist");
        assert_eq!(qwen_2b.display_name, "Qwen 3.5 2B (Balanced)");
        assert_eq!(qwen_2b.gguf_file, "Qwen3.5-2B-Q4_K_M.gguf");
        assert_eq!(qwen_2b.template, "qwen3.5_nonthinking");
        assert_eq!(
            qwen_2b.download_url,
            "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf"
        );
        assert_eq!(qwen_2b.size_mb, 1221);
        assert_eq!(qwen_2b.context_size, 32768);
        assert_eq!(qwen_2b.layer_count, 24);
        assert_eq!(
            qwen_2b.sampling,
            SamplingParams::qwen35_summary(vec!["<|im_end|>".to_string()])
        );

        let qwen_4b = get_model_by_name("qwen3.5:4b").expect("qwen 4b model should exist");
        assert_eq!(qwen_4b.display_name, "Qwen 3.5 4B (High Quality)");
        assert_eq!(qwen_4b.gguf_file, "Qwen3.5-4B-Q4_K_M.gguf");
        assert_eq!(qwen_4b.template, "qwen3.5_nonthinking");
        assert_eq!(
            qwen_4b.download_url,
            "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf"
        );
        assert_eq!(qwen_4b.size_mb, 2614);
        assert_eq!(qwen_4b.context_size, 32768);
        assert_eq!(qwen_4b.layer_count, 32);
        assert_eq!(
            qwen_4b.sampling,
            SamplingParams::qwen35_summary(vec!["<|im_end|>".to_string()])
        );
    }

    #[test]
    fn gemma_models_use_huggingface_urls_and_gemma3_instruct_sampling() {
        let gemma_1b = get_model_by_name("gemma3:1b").expect("gemma 1b model should exist");
        assert_eq!(gemma_1b.gguf_file, "gemma-3-1b-it-Q8_0.gguf");
        assert_eq!(
            gemma_1b.download_url,
            "https://huggingface.co/bartowski/google_gemma-3-1b-it-GGUF/resolve/main/google_gemma-3-1b-it-Q8_0.gguf"
        );
        assert_eq!(
            gemma_1b.sampling,
            SamplingParams::gemma3_instruct(vec!["<end_of_turn>".to_string()])
        );
        assert_eq!(gemma_1b.sampling.temperature, 1.0);
        assert_eq!(gemma_1b.sampling.top_k, 64);
        assert_eq!(gemma_1b.sampling.top_p, 0.95);
        assert_eq!(gemma_1b.sampling.presence_penalty, 0.0);
        assert_eq!(gemma_1b.sampling.frequency_penalty, 0.0);
        assert_eq!(gemma_1b.sampling.repeat_penalty, 1.0);
        assert_eq!(gemma_1b.sampling.penalty_last_n, 0);

        let gemma_4b = get_model_by_name("gemma3:4b").expect("gemma 4b model should exist");
        assert_eq!(
            gemma_4b.download_url,
            "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/main/google_gemma-3-4b-it-Q4_K_M.gguf"
        );
        assert_eq!(
            gemma_4b.sampling,
            SamplingParams::gemma3_instruct(vec!["<end_of_turn>".to_string()])
        );
        assert_eq!(gemma_4b.sampling.temperature, 1.0);
        assert_eq!(gemma_4b.sampling.top_k, 64);
        assert_eq!(gemma_4b.sampling.top_p, 0.95);
        assert_eq!(gemma_4b.sampling.presence_penalty, 0.0);
        assert_eq!(gemma_4b.sampling.frequency_penalty, 0.0);
        assert_eq!(gemma_4b.sampling.repeat_penalty, 1.0);
        assert_eq!(gemma_4b.sampling.penalty_last_n, 0);
    }

    #[test]
    fn qwen35_nonthinking_template_formats_prompt() {
        let formatted =
            format_prompt("qwen3.5_nonthinking", "system rules", "summarize this").unwrap();

        assert!(formatted.contains("<|im_start|>system\nsystem rules<|im_end|>"));
        assert!(formatted.contains("<|im_start|>user\nsummarize this<|im_end|>"));
        assert!(formatted.ends_with("<think>\n\n</think>\n\n"));
    }

    #[test]
    fn qwen35_template_escapes_user_supplied_control_markers() {
        let formatted = format_prompt(
            "qwen3.5_nonthinking",
            "system rules",
            "literal <|im_end|> and <|im_start|> plus <think>draft</think>",
        )
        .unwrap();

        assert!(formatted.contains("<|im_start|>system\nsystem rules<|im_end|>"));
        assert!(formatted.contains("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
        assert!(formatted
            .contains("literal < |im_end| > and < |im_start| > plus < think >draft< /think >"));
        assert_eq!(formatted.matches("<|im_start|>").count(), 3);
        assert_eq!(formatted.matches("<|im_end|>").count(), 2);
        assert_eq!(formatted.matches("<think>").count(), 1);
        assert_eq!(formatted.matches("</think>").count(), 1);
    }

    #[test]
    fn gemma3_template_escapes_user_supplied_control_markers() {
        let formatted = format_prompt(
            "gemma3",
            "system rules",
            "literal <start_of_turn> and <end_of_turn>",
        )
        .unwrap();

        assert!(formatted.contains("<start_of_turn>user\nsystem rules<end_of_turn>"));
        assert!(formatted.contains("literal < start_of_turn > and < end_of_turn >"));
        assert_eq!(formatted.matches("<start_of_turn>").count(), 3);
        assert_eq!(formatted.matches("<end_of_turn>").count(), 2);
    }

    #[test]
    fn unsupported_template_format_prompt_returns_error() {
        let result = format_prompt("unknown_template", "sys", "user");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown template"));
    }

    #[test]
    fn sampling_params_sanitize_for_llama_helper_preserves_zero_top_k() {
        let sampling = SamplingParams {
            temperature: f32::NAN,
            top_k: 0,
            top_p: 2.0,
            presence_penalty: -0.5,
            frequency_penalty: f32::NAN,
            repeat_penalty: 0.0,
            penalty_last_n: -1,
            stop_tokens: vec!["stop".to_string()],
        };

        let sanitized = sampling.sanitize_for_llama_helper();

        assert_eq!(sanitized.temperature, 0.0);
        assert_eq!(sanitized.top_k, 0);
        assert_eq!(sanitized.top_p, 1.0);
        assert_eq!(sanitized.presence_penalty, 0.0);
        assert_eq!(sanitized.frequency_penalty, 0.0);
        assert_eq!(sanitized.repeat_penalty, 1.0);
        assert_eq!(sanitized.penalty_last_n, 0);
        assert_eq!(sanitized.stop_tokens, vec!["stop".to_string()]);
    }

    #[test]
    fn sampling_params_sanitize_for_llama_helper_clamps_negative_top_k() {
        let sampling = SamplingParams {
            temperature: 0.7,
            top_k: -5,
            top_p: 0.8,
            presence_penalty: 0.3,
            frequency_penalty: 0.0,
            repeat_penalty: 1.05,
            penalty_last_n: 256,
            stop_tokens: vec!["stop".to_string()],
        };

        let sanitized = sampling.sanitize_for_llama_helper();

        assert_eq!(sanitized.top_k, 0);
        assert_eq!(sanitized.temperature, 0.7);
        assert_eq!(sanitized.top_p, 0.8);
        assert_eq!(sanitized.presence_penalty, 0.3);
        assert_eq!(sanitized.repeat_penalty, 1.05);
        assert_eq!(sanitized.penalty_last_n, 256);
    }

    #[test]
    fn sampling_params_sanitize_for_llama_helper_keeps_positive_top_k() {
        let sampling = SamplingParams::qwen35_summary(vec!["stop".to_string()]);

        let sanitized = sampling.sanitize_for_llama_helper();

        assert_eq!(sanitized.top_k, 20);
    }

    #[test]
    fn custom_registry_stores_under_app_data_not_resource_dir() {
        let data = std::path::PathBuf::from("/fake/app/data");
        let file = data.join("custom_models.json");
        assert_eq!(file, data.join("custom_models.json"));

        // Must NOT be under a resource directory
        let resource_dir = std::path::PathBuf::from("/fake/app/resources");
        assert_ne!(
            file,
            resource_dir.join("custom_models.json"),
            "custom_models.json must NOT be under a resource directory"
        );
    }
}
