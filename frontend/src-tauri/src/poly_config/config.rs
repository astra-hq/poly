use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::knowledge_graph::config::{
    EmbeddingConfig, KnowledgeGraphProfile, KnowledgeGraphSelection, ProfileKind,
};
use crate::providers::ProviderConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Summary config (references a provider from the global providers list)
// ─────────────────────────────────────────────────────────────────────────────

/// Non-secret summary configuration.
///
/// `provider_id` references an entry in the global `PolyConfig::providers` list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryConfig {
    /// ID of the provider to use for summaries (must exist in `providers` list).
    #[serde(default = "default_summary_provider_id")]
    pub provider_id: String,

    /// Model name used by the summary provider.
    #[serde(default = "default_summary_model")]
    pub model: String,

    /// Legacy whisper_model field — silently accepted during deserialization
    /// but NEVER serialized back to YAML.
    #[serde(default, skip_serializing)]
    pub _whisper_model: Option<String>,
}

fn default_summary_provider_id() -> String {
    "local".into()
}

fn default_summary_model() -> String {
    "gpt-4o-2024-11-20".into()
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            provider_id: default_summary_provider_id(),
            model: default_summary_model(),
            _whisper_model: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transcript config
// ─────────────────────────────────────────────────────────────────────────────

/// Non-secret transcript configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptConfig {
    /// Transcription provider (parakeet, deepgram, elevenLabs, groq, openai).
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
// Knowledge graph — secret-free profile wrapper
// ─────────────────────────────────────────────────────────────────────────────

/// A knowledge graph profile with **no** secrets suitable for YAML persistence.
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
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_provider_id: Option<String>,
}

fn default_llm_model() -> String {
    "qwen3:30b-a3b".to_string()
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
            llm_model: p.llm_model,
            llm_provider_id: p.llm_provider_id,
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
            llm_model: p.llm_model,
            llm_provider_id: p.llm_provider_id,
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
/// `providers` is a global list of LLM provider entities. Summary and KG
/// profiles reference providers by their `id`.
///
/// Raw API keys and other secrets are **never** serialized to this file;
/// they live in the separate `SecretStore`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolyConfig {
    /// Global list of LLM provider configurations.
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,

    #[serde(default)]
    pub summary: SummaryConfig,

    #[serde(default)]
    pub transcript: TranscriptConfig,

    #[serde(default, rename = "knowledge_graph")]
    pub knowledge_graph: KnowledgeGraphSettingsWithoutSecrets,

    #[serde(default)]
    pub preferences: PreferencesConfig,
}

impl Default for PolyConfig {
    fn default() -> Self {
        // Create sensible default providers
        Self {
            providers: vec![ProviderConfig {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                provider_type: crate::providers::ProviderType::OpenAI,
                base_url: "https://api.openai.com/v1".to_string(),
                default_model: "gpt-4o".to_string(),
            }],
            summary: SummaryConfig {
                provider_id: "local".to_string(),
                model: "gpt-4o-2024-11-20".to_string(),
                _whisper_model: None,
            },
            transcript: TranscriptConfig::default(),
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

    /// Parse a YAML string, migrating stale legacy provider IDs to `local`.
    pub fn load_from_str(yaml: &str) -> Result<Self> {
        let mut config: Self = serde_yaml::from_str(yaml)
            .with_context(|| "Failed to parse YAML config".to_string())?;
        // Migrate the legacy built-in-AI provider ID (0.x compat).
        if config.summary.provider_id == "builtin-ai" {
            config.summary.provider_id = "local".to_string();
        }
        Ok(config)
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

    /// Find a provider by ID.
    pub fn find_provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_summary_matches_app_defaults() {
        let s = SummaryConfig::default();
        assert_eq!(s.provider_id, "local");
        assert_eq!(s.model, "gpt-4o-2024-11-20");
    }

    #[test]
    fn default_transcript_matches_app_defaults() {
        let t = TranscriptConfig::default();
        assert_eq!(t.provider, "parakeet");
        assert_eq!(t.model, crate::config::DEFAULT_PARAKEET_MODEL);
    }

    #[test]
    fn default_poly_config_has_default_providers() {
        let cfg = PolyConfig::default();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.providers[0].id, "openai");
    }

    #[test]
    fn find_provider_by_id() {
        let cfg = PolyConfig::default();
        let p = cfg.find_provider("openai").unwrap();
        assert_eq!(p.name, "OpenAI");
        assert!(cfg.find_provider("nonexistent").is_none());
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
            llm_model: "qwen3:30b-a3b".into(),
            llm_provider_id: None,
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
        let yaml = "summary:\n  provider_id: groq\n";
        let cfg: PolyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.summary.provider_id, "groq");
        assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
        assert_eq!(cfg.transcript.provider, "parakeet");
        assert_eq!(cfg.preferences.language, "auto-translate");
    }

    #[test]
    fn new_default_uses_local() {
        let cfg = PolyConfig::default();
        assert_eq!(cfg.summary.provider_id, "local");
    }

    #[test]
    fn new_whisper_model_absent_from_default() {
        let s = SummaryConfig::default();
        let yaml = serde_yaml::to_string(&s).unwrap();
        assert!(
            !yaml.contains("whisper_model"),
            "whisper_model must not be serialized: got\n{}",
            yaml
        );
    }

    #[test]
    fn legacy_provider_id_migrates_to_local() {
        const YAML: &str = "summary:\n  provider_id: builtin-ai\n";
        let cfg = PolyConfig::load_from_str(YAML).unwrap();
        assert_eq!(cfg.summary.provider_id, "local");
    }

    #[test]
    fn old_yaml_with_whisper_model_still_parses() {
        let yaml = "summary:\n  provider_id: local\n  whisper_model: large-v3\n";
        let cfg = PolyConfig::load_from_str(yaml).unwrap();
        assert_eq!(cfg.summary.provider_id, "local");
    }

    #[test]
    fn default_config_has_no_ollama_dependency() {
        let cfg = PolyConfig::default();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        // Summary defaults to "local", never "ollama"
        assert_eq!(cfg.summary.provider_id, "local");
        // Transcript defaults to "parakeet", never "localWhisper"
        assert_eq!(cfg.transcript.provider, "parakeet");
        assert!(
            !yaml.contains("ollama"),
            "ollama must not appear in default: {}",
            yaml
        );
    }

    #[test]
    fn default_config_has_no_whisper_transcription_path() {
        let cfg = PolyConfig::default();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        assert_eq!(cfg.transcript.provider, "parakeet");
        assert!(
            !yaml.contains("localWhisper"),
            "localWhisper must not appear in default: {}",
            yaml
        );
        assert!(
            !yaml.contains("whisper_model"),
            "whisper_model must not appear in default: {}",
            yaml
        );
    }

    #[test]
    fn builtin_ai_is_not_an_active_provider_alias() {
        let cfg = PolyConfig::default();
        assert_ne!(cfg.summary.provider_id, "builtin-ai");
        assert_ne!(cfg.transcript.provider, "builtin-ai");
        // "builtin-ai" in YAML is migrated to "local" at load time
        let yaml = "summary:\n  provider_id: builtin-ai\n";
        let migrated = PolyConfig::load_from_str(yaml).unwrap();
        assert_eq!(migrated.summary.provider_id, "local");
    }
}
