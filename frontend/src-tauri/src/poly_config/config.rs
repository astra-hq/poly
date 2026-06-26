use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::knowledge_graph::config::{
    EmbeddingConfig, KnowledgeGraphProfile, KnowledgeGraphSelection, ProfileKind,
};

// ─────────────────────────────────────────────────────────────────────────────
// Summary config (non-secret portion of settings)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-secret summary configuration.
///
/// Maps to the `settings` table's `provider`, `model`, `whisperModel`,
/// and `ollamaEndpoint` columns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryConfig {
    /// AI provider for summaries (openai, claude, ollama, groq, openrouter, custom-openai).
    #[serde(default = "default_summary_provider")]
    pub provider: String,

    /// Model name used by the summary provider.
    #[serde(default = "default_summary_model")]
    pub model: String,

    /// Whisper model used for transcription (local or remote).
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,

    /// Base URL for a local Ollama instance (only meaningful when provider is "ollama").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ollama_endpoint: Option<String>,
}

fn default_summary_provider() -> String {
    "openai".into()
}

fn default_summary_model() -> String {
    "gpt-4o-2024-11-20".into()
}

fn default_whisper_model() -> String {
    crate::config::DEFAULT_WHISPER_MODEL.into()
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            provider: default_summary_provider(),
            model: default_summary_model(),
            whisper_model: default_whisper_model(),
            ollama_endpoint: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transcript config (non-secret portion of transcript_settings)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-secret transcript configuration.
///
/// Maps to `transcript_settings.provider` and `transcript_settings.model`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptConfig {
    /// Transcription provider (localWhisper, parakeet, deepgram, elevenLabs,
    /// groq, openai).
    #[serde(default = "default_transcript_provider")]
    pub provider: String,

    /// Model name used for transcription.
    #[serde(default = "default_transcript_model")]
    pub model: String,
}

fn default_transcript_provider() -> String {
    "parakeet".into()
}

fn default_transcript_model() -> String {
    crate::config::DEFAULT_PARAKEET_MODEL.into()
}

impl Default for TranscriptConfig {
    fn default() -> Self {
        Self {
            provider: default_transcript_provider(),
            model: default_transcript_model(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Custom OpenAI config (non-secret fields of CustomOpenAIConfig)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-secret fields of a custom OpenAI-compatible endpoint configuration.
///
/// The `api_key` field from the database's `CustomOpenAIConfig` is deliberately
/// excluded — it belongs in the `SecretStore`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomOpenAIConfigFields {
    /// Base URL of the OpenAI-compatible API endpoint.
    #[serde(default)]
    pub endpoint: String,

    /// Model identifier (e.g., "gpt-4", "llama-3-70b").
    #[serde(default)]
    pub model: String,

    /// Maximum tokens for completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,

    /// Temperature parameter (0.0–2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-P sampling parameter (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

impl Default for CustomOpenAIConfigFields {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            model: String::new(),
            max_tokens: None,
            temperature: None,
            top_p: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Knowledge graph — secret-free profile wrapper
// ─────────────────────────────────────────────────────────────────────────────

/// A knowledge graph profile with **no** secrets suitable for YAML persistence.
///
/// This is a deliberate wrapper around [`KnowledgeGraphProfile`] that
/// completely omits the `api_key` field.  All other behavioural and
/// display fields are preserved 1:1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeGraphProfileWithoutSecrets {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: ProfileKind,
    #[serde(default)]
    pub embedding: EmbeddingConfig,
    pub lightrag_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl From<KnowledgeGraphProfile> for KnowledgeGraphProfileWithoutSecrets {
    fn from(p: KnowledgeGraphProfile) -> Self {
        Self {
            id: p.id,
            name: p.name,
            kind: p.kind,
            embedding: p.embedding,
            lightrag_url: p.lightrag_url,
            notes: p.notes,
        }
    }
}

impl From<KnowledgeGraphProfileWithoutSecrets> for KnowledgeGraphProfile {
    fn from(p: KnowledgeGraphProfileWithoutSecrets) -> Self {
        Self {
            id: p.id,
            name: p.name,
            kind: p.kind,
            embedding: p.embedding,
            lightrag_url: p.lightrag_url,
            api_key: None, // never persisted — caller must merge from SecretStore
            notes: p.notes,
            has_secret: false,
            api_key_masked_hint: None,
        }
    }
}

/// Secret-free knowledge graph settings suitable for YAML persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeGraphSettingsWithoutSecrets {
    #[serde(default)]
    pub profiles: Vec<KnowledgeGraphProfileWithoutSecrets>,

    #[serde(default)]
    pub active_profile: KnowledgeGraphSelection,
}

impl Default for KnowledgeGraphSettingsWithoutSecrets {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile: KnowledgeGraphSelection::None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preferences
// ─────────────────────────────────────────────────────────────────────────────

/// User preferences that are not provider-specific.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreferencesConfig {
    /// Language preference for summaries/translation.
    ///
    /// Canonical values: `"auto-translate"`, `"en"`, `"fr"`, …
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "auto-translate".into()
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level config
// ─────────────────────────────────────────────────────────────────────────────

/// The authoritative non-secret configuration for the Poly desktop app.
///
/// This is persisted as YAML at `~/.poly/poly.yml` and contains **every**
/// non-secret configuration value.
///
/// For users upgrading from the legacy Resourcefully name, the config
/// repository automatically migrates `~/.resourcefully/resourcefully.yml`
/// to the current path on first use without deleting the legacy file.
///
/// Raw API keys and other secrets are **never** serialized to this file;
/// they live in the separate `SecretStore`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolyConfig {
    #[serde(default)]
    pub summary: SummaryConfig,

    #[serde(default)]
    pub transcript: TranscriptConfig,

    #[serde(default, rename = "custom_openai")]
    pub custom_openai: CustomOpenAIConfigFields,

    #[serde(default, rename = "knowledge_graph")]
    pub knowledge_graph: KnowledgeGraphSettingsWithoutSecrets,

    #[serde(default)]
    pub preferences: PreferencesConfig,
}

impl Default for PolyConfig {
    fn default() -> Self {
        Self {
            summary: SummaryConfig::default(),
            transcript: TranscriptConfig::default(),
            custom_openai: CustomOpenAIConfigFields::default(),
            knowledge_graph: KnowledgeGraphSettingsWithoutSecrets::default(),
            preferences: PreferencesConfig::default(),
        }
    }
}

impl PolyConfig {
    // ── constructors ────────────────────────────────────────────────────

    /// Return the default configuration (in-memory only).
    pub fn load_default() -> Self {
        Self::default()
    }

    /// Load from the default path (`~/.poly/poly.yml`).
    ///
    /// Returns the default config if the file does not exist.
    pub fn load() -> Result<Self> {
        Self::load_from_path(&super::paths::default_config_path())
    }

    /// Load from an explicit file path.
    ///
    /// Returns the default config if the file does not exist.
    pub fn load_from_path(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::load_from_str(&contents),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::anyhow!("Failed to read config file: {}", e)),
        }
    }

    /// Parse a YAML string.
    pub fn load_from_str(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml).with_context(|| "Failed to parse YAML config".to_string())
    }

    // ── persistence ─────────────────────────────────────────────────────

    /// Serialize to the default path (`~/.poly/poly.yml`).
    pub fn save(&self) -> Result<()> {
        self.save_to_path(&super::paths::default_config_path())
    }

    /// Serialize the default configuration to the default path.
    pub fn save_default() -> Result<()> {
        Self::default().save()
    }

    /// Serialize to an explicit file path.
    pub fn save_to_path(&self, path: &std::path::Path) -> Result<()> {
        super::paths::ensure_config_dir().with_context(|| "Failed to create config directory")?;

        let yaml =
            serde_yaml::to_string(self).with_context(|| "Failed to serialize config to YAML")?;

        std::fs::write(path, yaml)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests (basic.  Full integration tests are in
// `tests/poly_config_test.rs`.)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_summary_matches_app_defaults() {
        let s = SummaryConfig::default();
        assert_eq!(s.provider, "openai");
        assert_eq!(s.model, "gpt-4o-2024-11-20");
        assert_eq!(s.whisper_model, crate::config::DEFAULT_WHISPER_MODEL);
    }

    #[test]
    fn default_transcript_matches_app_defaults() {
        let t = TranscriptConfig::default();
        assert_eq!(t.provider, "parakeet");
        assert_eq!(t.model, crate::config::DEFAULT_PARAKEET_MODEL);
    }

    #[test]
    fn kg_profile_roundtrip_strips_api_key() {
        let original = KnowledgeGraphProfile {
            id: "x".into(),
            name: "x".into(),
            kind: ProfileKind::Remote,
            embedding: EmbeddingConfig::default(),
            lightrag_url: "http://localhost:9621".into(),
            api_key: Some("secret".into()),
            notes: Some("note".into()),
            has_secret: false,
            api_key_masked_hint: None,
        };

        let without = KnowledgeGraphProfileWithoutSecrets::from(original);
        let back: KnowledgeGraphProfile = without.into();

        assert_eq!(back.id, "x");
        assert_eq!(back.name, "x");
        assert_eq!(back.kind, ProfileKind::Remote);
        assert_eq!(back.notes.as_deref(), Some("note"));
        assert!(
            back.api_key.is_none(),
            "api_key must be None after roundtrip"
        );
    }

    #[test]
    fn deserialize_minimal_yaml() {
        let yaml = "summary:\n  provider: groq\n";
        let cfg: PolyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.summary.provider, "groq");
        // All other fields should be defaults
        assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
        assert_eq!(cfg.transcript.provider, "parakeet");
        assert_eq!(cfg.preferences.language, "auto-translate");
    }
}
