use app_lib::knowledge_graph::config::KnowledgeGraphSelection;
use app_lib::poly_config::config::{
    KnowledgeGraphProfileWithoutSecrets, KnowledgeGraphSettingsWithoutSecrets, PolyConfig,
    PreferencesConfig, SummaryConfig, TranscriptConfig,
};
use app_lib::providers::{ProviderConfig, ProviderType};

// ─── full schema roundtrip test ────────────────────────────────────────────

#[test]
fn poly_config_full_schema_roundtrip_without_raw_secrets() {
    let cfg = PolyConfig {
        providers: vec![
            ProviderConfig {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                provider_type: ProviderType::OpenAI,
                base_url: "https://api.openai.com/v1".to_string(),
                default_model: "gpt-4o".to_string(),
            },
            ProviderConfig {
                id: "custom-ai".to_string(),
                name: "Custom AI".to_string(),
                provider_type: ProviderType::Custom,
                base_url: "https://api.custom-ai.example.com/v1".to_string(),
                default_model: "custom-model-v2".to_string(),
            },
            ProviderConfig {
                id: "ollama-local".to_string(),
                name: "Local Ollama".to_string(),
                provider_type: ProviderType::Ollama,
                base_url: "http://localhost:11434".to_string(),
                default_model: "llama3.1:8b".to_string(),
            },
        ],
        summary: SummaryConfig {
            provider_id: "openai".to_string(),
            model: "gpt-4o-2024-11-20".to_string(),
            _whisper_model: Some("large-v3-turbo".to_string()),
        },
        transcript: TranscriptConfig {
            provider: "parakeet".to_string(),
            model: "parakeet-tdt-0.6b-v3-int8".to_string(),
        },
        knowledge_graph: KnowledgeGraphSettingsWithoutSecrets {
            profiles: vec![KnowledgeGraphProfileWithoutSecrets {
                id: "kg-1".to_string(),
                name: "Production KG".to_string(),
                kind: app_lib::knowledge_graph::config::ProfileKind::Remote,
                embedding: app_lib::knowledge_graph::config::EmbeddingConfig {
                    provider: "openai".to_string(),
                    model: "text-embedding-3-large".to_string(),
                    dimensions: 3072,
                },
                lightrag_url: "https://kg.example.com:9621".to_string(),
                notes: Some("Main production knowledge graph".to_string()),
                llm_model: "qwen3:30b-a3b".to_string(),
                llm_provider_id: Some("custom-ai".to_string()),
            }],
            active_profile: KnowledgeGraphSelection::Profile("kg-1".to_string()),
        },
        preferences: PreferencesConfig {
            language: "en".to_string(),
        },
    };

    let yaml = serde_yaml::to_string(&cfg).unwrap();

    // Assert expected fields are present
    assert!(yaml.contains("openai"));
    assert!(yaml.contains("gpt-4o-2024-11-20"));
    assert!(yaml.contains("http://localhost:11434"));
    assert!(yaml.contains("parakeet"));
    assert!(yaml.contains("custom-model-v2"));
    assert!(yaml.contains("https://api.custom-ai.example.com/v1"));
    assert!(yaml.contains("custom-ai"));
    assert!(yaml.contains("kg-1"));
    assert!(yaml.contains("Production KG"));
    assert!(yaml.contains("https://kg.example.com:9621"));
    assert!(yaml.contains("text-embedding-3-large"));
    assert!(yaml.contains("en"));

    // Assert NO raw secrets in YAML
    let lower = yaml.to_lowercase();
    assert!(!lower.contains("api_key"), "api_key leaked into YAML");
    assert!(!lower.contains("apikey"), "apikey leaked into YAML");
    assert!(!lower.contains("secret_key"), "secret_key leaked into YAML");
    assert!(
        !lower.contains("access_token"),
        "access_token leaked into YAML"
    );

    // Roundtrip: deserialize and compare
    let restored: PolyConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(restored.providers, cfg.providers);
    assert_eq!(restored.summary.provider_id, cfg.summary.provider_id);
    assert_eq!(restored.summary.model, cfg.summary.model);
    assert_eq!(restored.summary._whisper_model, None); // skip_serializing
    assert_eq!(restored.transcript.provider, cfg.transcript.provider);
    assert_eq!(restored.transcript.model, cfg.transcript.model);
    assert_eq!(restored.knowledge_graph.profiles.len(), 1);
    assert_eq!(restored.knowledge_graph.profiles[0].id, "kg-1");
    assert_eq!(
        restored.knowledge_graph.profiles[0]
            .llm_provider_id
            .as_deref(),
        Some("custom-ai")
    );
    assert_eq!(
        restored.knowledge_graph.active_profile,
        KnowledgeGraphSelection::Profile("kg-1".to_string())
    );
    assert_eq!(restored.preferences.language, "en");
}

// ─── partial config loading test ───────────────────────────────────────────

#[test]
fn partial_config_loading_with_defaults() {
    // Only set a few fields - everything else should use defaults
    let partial_yaml = r#"
summary:
  provider_id: claude
  model: claude-3-opus
transcript:
  provider: groq
preferences:
  language: fr
"#;

    let cfg: PolyConfig = serde_yaml::from_str(partial_yaml).unwrap();

    // Set fields
    assert_eq!(cfg.summary.provider_id, "claude");
    assert_eq!(cfg.summary.model, "claude-3-opus");
    assert_eq!(cfg.transcript.provider, "groq");

    // Defaulted fields
    assert_eq!(
        cfg.summary._whisper_model,
        None
    ); // default
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8"); // default
    assert!(cfg.providers.is_empty());
    assert!(cfg.knowledge_graph.profiles.is_empty()); // default
    assert_eq!(
        cfg.knowledge_graph.active_profile,
        KnowledgeGraphSelection::None
    ); // default
    assert_eq!(cfg.preferences.language, "fr"); // set
}

// ─── completely empty YAML ─────────────────────────────────────────────────

#[test]
fn empty_yaml_produces_all_defaults() {
    let yaml = "{}";
    let cfg: PolyConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(cfg.summary.provider_id, "local");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(
        cfg.summary._whisper_model,
        None
    );
    assert_eq!(cfg.transcript.provider, "local");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert!(cfg.providers.is_empty());
    assert!(cfg.knowledge_graph.profiles.is_empty());
    assert_eq!(
        cfg.knowledge_graph.active_profile,
        KnowledgeGraphSelection::None
    );
    assert_eq!(cfg.preferences.language, "auto-translate");
}

// ─── malformed YAML returns typed error without deleting file ───────────────

#[test]
fn malformed_yaml_returns_error_without_deleting_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("bad_config.yml");

    // Write malformed YAML
    std::fs::write(&file_path, "summary: [unclosed\n  provider: openai\n").unwrap();

    let contents = std::fs::read_to_string(&file_path).unwrap();
    let result = PolyConfig::load_from_str(&contents);

    assert!(result.is_err(), "Malformed YAML should produce an error");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("parse")
            || err.to_string().contains("YAML")
            || err.to_string().contains("yaml"),
        "Error should mention parse/yaml: got '{}'",
        err
    );

    // File still exists and contents unchanged
    let after = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(after, "summary: [unclosed\n  provider: openai\n");
}

// ─── save and load from file ────────────────────────────────────────────────

#[test]
fn save_and_load_roundtrip_via_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("config.yml");

    let cfg = PolyConfig {
        providers: vec![ProviderConfig {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            provider_type: ProviderType::Ollama,
            base_url: "http://192.168.1.100:11434".to_string(),
            default_model: "llama3.1:8b".to_string(),
        }],
        summary: SummaryConfig {
            provider_id: "ollama".to_string(),
            model: "llama3.1:8b".to_string(),
            _whisper_model: Some("medium".to_string()),
        },
        transcript: TranscriptConfig {
            provider: "localWhisper".to_string(),
            model: "large-v3".to_string(),
        },
        knowledge_graph: KnowledgeGraphSettingsWithoutSecrets {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        },
        preferences: PreferencesConfig {
            language: "ja".to_string(),
        },
    };

    // Save
    let yaml = serde_yaml::to_string(&cfg).unwrap();
    std::fs::write(&file_path, &yaml).unwrap();

    // Load
    let read_back = std::fs::read_to_string(&file_path).unwrap();
    let restored: PolyConfig = serde_yaml::from_str(&read_back).unwrap();

    assert_eq!(restored.providers.len(), 1);
    assert_eq!(restored.providers[0].id, "ollama");
    assert_eq!(restored.providers[0].base_url, "http://192.168.1.100:11434");
    assert_eq!(restored.summary.provider_id, "ollama");
    assert_eq!(restored.summary.model, "llama3.1:8b");
    assert_eq!(restored.summary._whisper_model, None);
    assert_eq!(restored.transcript.provider, "localWhisper");
    assert_eq!(restored.transcript.model, "large-v3");
    assert!(restored.knowledge_graph.profiles.is_empty());
    assert_eq!(restored.preferences.language, "ja");
}

// ─── KG profile serialization excludes api_key ─────────────────────────────

#[test]
fn kg_profile_without_secrets_excludes_api_key() {
    use app_lib::knowledge_graph::config::KnowledgeGraphProfile;

    let original = KnowledgeGraphProfile {
        id: "secret-profile".to_string(),
        name: "With Secret".to_string(),
        kind: app_lib::knowledge_graph::config::ProfileKind::Remote,
        embedding: app_lib::knowledge_graph::config::EmbeddingConfig {
            provider: "mlx".to_string(),
            model: "BAAI/bge-m3".to_string(),
            dimensions: 1024,
        },
        lightrag_url: "http://localhost:9621".to_string(),
        api_key: Some("sk-super-secret-key-12345".to_string()),
        notes: Some("Has an API key".to_string()),
        has_secret: false,
        api_key_masked_hint: None,
        llm_model: "qwen3:30b-a3b".to_string(),
        llm_provider_id: Some("openai".to_string()),
    };

    let without: KnowledgeGraphProfileWithoutSecrets = original.into();
    let yaml = serde_yaml::to_string(&without).unwrap();

    assert!(yaml.contains("secret-profile"));
    assert!(yaml.contains("With Secret"));
    assert!(yaml.contains("Has an API key"));
    assert!(
        !yaml.contains("sk-super-secret-key-12345"),
        "API key leaked into YAML: {}",
        yaml
    );
    assert!(
        !yaml.to_lowercase().contains("api_key"),
        "api_key field leaked: {}",
        yaml
    );
}

// ─── load_default / save_default convenience methods ───────────────────────

#[test]
fn load_default_returns_default_config() {
    let cfg = PolyConfig::load_default();
    assert_eq!(cfg.summary.provider_id, "local");
    assert_eq!(cfg.preferences.language, "auto-translate");
}

#[test]
fn load_from_file_missing_returns_default() {
    let cfg = PolyConfig::load_from_path(&std::path::PathBuf::from("/nonexistent/path/poly.yml"));
    assert!(cfg.is_ok(), "Missing file should return default, not error");
    let cfg = cfg.unwrap();
    assert_eq!(cfg.summary.provider_id, "local");
}

// ─── Default implementations ───────────────────────────────────────────────

#[test]
fn default_poly_config_has_sensible_values() {
    let cfg = PolyConfig::default();
    assert_eq!(cfg.summary.provider_id, "local");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(cfg.summary._whisper_model, None);
    assert_eq!(cfg.transcript.provider, "local");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(cfg.preferences.language, "auto-translate");
}

// ─── atomic save preserves existing file on failure ───────────────────────

#[test]
fn poly_config_atomic_save_preserves_existing_file_on_failure() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("config.yml");
    let repo = ConfigRepository::with_path(file_path.clone());

    // Write an original config file.
    let original_cfg = PolyConfig {
        providers: PolyConfig::default().providers,
        summary: SummaryConfig {
            provider_id: "openai".to_string(),
            model: "original-model".to_string(),
            ..Default::default()
        },
        transcript: TranscriptConfig::default(),
        knowledge_graph: KnowledgeGraphSettingsWithoutSecrets::default(),
        preferences: PreferencesConfig::default(),
    };
    let yaml = serde_yaml::to_string(&original_cfg).unwrap();
    std::fs::write(&file_path, &yaml).unwrap();

    let original_bytes = fs::read(&file_path).unwrap();

    // Make the directory read-only to prevent temp file creation.
    // 0o555 = r-xr-xr-x (readable + traversable, not writable).
    let dir_path = dir.path().to_path_buf();
    fs::set_permissions(&dir_path, Permissions::from_mode(0o555)).unwrap();

    // Attempt an atomic save — must fail.
    let new_cfg = PolyConfig {
        providers: vec![ProviderConfig {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            default_model: "claude-3-opus".to_string(),
        }],
        summary: SummaryConfig {
            provider_id: "claude".to_string(),
            model: "new-model".to_string(),
            _whisper_model: Some("large-v3-turbo".to_string()),
        },
        transcript: TranscriptConfig::default(),
        knowledge_graph: KnowledgeGraphSettingsWithoutSecrets::default(),
        preferences: PreferencesConfig::default(),
    };
    let result = repo.save_atomic(&new_cfg);
    assert!(
        result.is_err(),
        "save_atomic must fail when directory is read-only"
    );

    // Restore permissions so tempdir can clean up on drop.
    let _ = fs::set_permissions(&dir_path, Permissions::from_mode(0o755));

    // Original file must be unchanged byte-for-byte.
    let after_bytes = fs::read(&file_path).unwrap();
    assert_eq!(
        after_bytes, original_bytes,
        "Original config file must be preserved byte-for-byte after failed atomic save"
    );

    // Loading the file must still yield the original config.
    let loaded = repo.load().unwrap();
    assert_eq!(loaded, original_cfg);
}

// ─── startup creates default YAML without config tables ─────────────────

#[test]
fn startup_creates_default_poly_yaml_without_config_tables() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    // Simulate a fresh config directory (no YAML, no legacy SQLite config tables)
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("resourcefully.yml");
    let repo = ConfigRepository::with_path(file_path.clone());

    // Fresh start: no YAML file should exist
    assert!(
        !file_path.exists(),
        "Fresh start: no YAML config should exist yet"
    );

    // Startup bootstrap: load_or_create_default writes defaults on first run
    let cfg = repo.load_or_create_default().unwrap();

    // File was created
    assert!(
        file_path.exists(),
        "load_or_create_default must create YAML file on first run"
    );

    // Returned config has correct defaults
    assert_eq!(cfg.summary.provider_id, "local");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(
        cfg.summary._whisper_model,
        None
    );
    assert_eq!(cfg.transcript.provider, "local");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(cfg.preferences.language, "auto-translate");

    // Verify actual file contents
    let contents = fs::read_to_string(&file_path).unwrap();
    assert!(
        contents.contains("local"),
        "YAML must contain default summary provider"
    );
    assert!(
        contents.contains("parakeet-tdt-0.6b-v3-int8"),
        "YAML must contain transcript model"
    );
    assert!(
        contents.contains("auto-translate"),
        "YAML must contain default language"
    );

    // Second startup: file already exists — must NOT be overwritten
    let mtime_before = fs::metadata(&file_path).unwrap().modified().unwrap();
    let cfg2 = repo.load_or_create_default().unwrap();
    let mtime_after = fs::metadata(&file_path).unwrap().modified().unwrap();

    assert_eq!(
        mtime_before, mtime_after,
        "Existing YAML must not be overwritten on subsequent loads"
    );
    assert_eq!(
        cfg2.summary.provider_id, cfg.summary.provider_id,
        "Reloaded config must match original"
    );
    assert_eq!(cfg2.transcript.provider, cfg.transcript.provider);
}

// ─── legacy migration integration tests ────────────────────────────────────

/// When no Poly config exists but a valid legacy config does, the legacy
/// config is loaded and atomically copied forward to the Poly path.
/// The legacy file is never deleted.
#[test]
fn legacy_config_copied_forward_to_poly_on_first_run() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy.yml");

    // Write a custom legacy config.
    let legacy_yaml = r#"
summary:
  provider_id: ollama
  model: llama3.1:8b
  whisper_model: medium
transcript:
  provider: localWhisper
  model: large-v3
preferences:
  language: de
"#;
    fs::write(&legacy_path, legacy_yaml).unwrap();

    // Poly file does not exist yet.
    assert!(!poly_path.exists());

    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let cfg = repo.load_or_create_default().unwrap();

    // Legacy values were loaded and migrated.
    assert_eq!(cfg.summary.provider_id, "ollama");
    assert_eq!(cfg.summary.model, "llama3.1:8b");
    assert_eq!(cfg.summary._whisper_model, None);
    assert_eq!(cfg.transcript.provider, "localWhisper");
    assert_eq!(cfg.transcript.model, "large-v3");
    assert_eq!(cfg.preferences.language, "de");

    // Poly file was created atomically.
    assert!(poly_path.exists());

    // Legacy file is preserved (never deleted).
    assert!(legacy_path.exists());
    let legacy_after = fs::read_to_string(&legacy_path).unwrap();
    assert_eq!(
        legacy_after, legacy_yaml,
        "legacy file must not be modified"
    );
}

/// When no Poly config exists and the legacy file contains malformed YAML,
/// `load_or_create_default` must return a typed error — it must NOT
/// silently generate a default config.
#[test]
fn malformed_legacy_yaml_returns_error_no_silent_default() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy_broken.yml");

    // Write malformed YAML.
    fs::write(&legacy_path, "summary: [unclosed\n  provider: openai\n").unwrap();

    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let result = repo.load_or_create_default();

    // Must return an error.
    assert!(
        result.is_err(),
        "malformed legacy YAML must produce an error, not a default config"
    );
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("parse") || msg.contains("YAML") || msg.contains("yaml"),
        "error must mention parse/yaml: got '{}'",
        msg
    );

    // Poly config must NOT have been written.
    assert!(
        !poly_path.exists(),
        "Poly config must not be written when legacy parse fails"
    );

    // Legacy file must still exist (not deleted).
    assert!(
        legacy_path.exists(),
        "legacy file must not be deleted on parse failure"
    );
}

/// When both Poly and legacy config files exist, the Poly file content
/// takes precedence. The legacy file is never consulted.
#[test]
fn poly_config_preferred_when_both_files_exist() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy.yml");

    // Write Poly config with one provider.
    fs::write(
        &poly_path,
        r#"
summary:
  provider_id: openai
  model: gpt-4o
transcript:
  provider: parakeet
"#,
    )
    .unwrap();

    // Write legacy config with a conflicting provider.
    fs::write(
        &legacy_path,
        r#"
summary:
  provider_id: claude
  model: claude-3-opus
transcript:
  provider: groq
"#,
    )
    .unwrap();

    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let cfg = repo.load_or_create_default().unwrap();

    // Poly values win.
    assert_eq!(cfg.summary.provider_id, "openai");
    assert_eq!(cfg.summary.model, "gpt-4o");
    assert_eq!(cfg.transcript.provider, "parakeet");

    // Legacy file untouched.
    assert!(legacy_path.exists());
}

/// When Poly config already exists, `load_or_create_default` must not
/// overwrite it with legacy data or defaults. The existing Poly file
/// is loaded as-is and returned unchanged.
#[test]
fn existing_poly_not_overwritten_by_legacy_migration() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy.yml");

    // Write Poly config with specific values.
    let poly_content = r#"
summary:
  provider_id: ollama
  model: custom-model
preferences:
  language: ja
"#;
    fs::write(&poly_path, poly_content).unwrap();
    let poly_mtime = fs::metadata(&poly_path).unwrap().modified().unwrap();

    // Write legacy config with completely different values.
    fs::write(
        &legacy_path,
        r#"
summary:
  provider_id: groq
  model: some-other-model
preferences:
  language: es
"#,
    )
    .unwrap();

    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let cfg = repo.load_or_create_default().unwrap();

    // Poly values unchanged — legacy was not consulted.
    assert_eq!(cfg.summary.provider_id, "ollama");
    assert_eq!(cfg.summary.model, "custom-model");
    assert_eq!(cfg.preferences.language, "ja");

    // Poly file not rewritten (mtime unchanged).
    let poly_mtime_after = fs::metadata(&poly_path).unwrap().modified().unwrap();
    assert_eq!(
        poly_mtime, poly_mtime_after,
        "existing Poly file must not be re-written"
    );

    // Legacy file untouched.
    assert!(legacy_path.exists());
}

/// When Poly config exists and `load()` is used directly (bypassing
/// migration), the Poly file is loaded and legacy is ignored.
#[test]
fn load_directly_always_reads_poly_path_not_legacy() {
    use app_lib::poly_config::repository::ConfigRepository;
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy.yml");

    fs::write(
        &poly_path,
        r#"
summary:
  provider_id: openai
"#,
    )
    .unwrap();

    fs::write(
        &legacy_path,
        r#"
summary:
  provider_id: claude
"#,
    )
    .unwrap();

    // Use with_paths so the repo has access to legacy path, but
    // load() only reads from the Poly path.
    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let cfg = repo.load().unwrap();

    assert_eq!(cfg.summary.provider_id, "openai");
}

// ─── namespace verification: Poly paths use the correct prefixes ────────

#[test]
fn default_config_path_uses_poly_namespace() {
    let path = app_lib::poly_config::paths::default_config_path();
    assert!(
        path.ends_with(".poly/poly.yml"),
        "default config path must be ~/.poly/poly.yml, got: {:?}",
        path
    );
    assert!(
        !path.to_string_lossy().contains(".resourcefully"),
        "default config path must NOT reference legacy .resourcefully namespace"
    );
}

#[test]
fn legacy_config_path_uses_resourcefully_namespace() {
    let path = app_lib::poly_config::paths::legacy_config_path();
    assert!(
        path.ends_with(".resourcefully/resourcefully.yml"),
        "legacy config path must be ~/.resourcefully/resourcefully.yml, got: {:?}",
        path
    );
}

#[test]
fn default_config_path_differs_from_legacy_config_path() {
    let poly = app_lib::poly_config::paths::default_config_path();
    let legacy = app_lib::poly_config::paths::legacy_config_path();
    assert_ne!(poly, legacy, "Poly and legacy config paths must differ");
}

#[test]
fn poly_media_dir_uses_poly_namespace() {
    let path = app_lib::app_data::paths::poly_media_dir();
    assert!(
        path.ends_with(".poly/media"),
        "Poly media dir must be ~/.poly/media, got: {:?}",
        path
    );
}

#[test]
fn legacy_media_dir_uses_resourcefully_namespace() {
    let path = app_lib::app_data::paths::legacy_media_dir();
    assert!(
        path.ends_with(".resourcefully/media"),
        "legacy media dir must be ~/.resourcefully/media, got: {:?}",
        path
    );
}

#[test]
fn legacy_media_dir_is_fallback_not_primary() {
    let poly = app_lib::app_data::paths::poly_media_dir();
    let legacy = app_lib::app_data::paths::legacy_media_dir();
    assert_ne!(poly, legacy, "Poly and legacy media dirs must differ");
    assert!(
        legacy.to_string_lossy().contains(".resourcefully"),
        "legacy media dir must reference .resourcefully namespace"
    );
}

#[test]
fn legacy_config_path_appears_only_as_fallback_not_as_primary() {
    let primary = app_lib::poly_config::paths::default_config_path();
    let fallback = app_lib::poly_config::paths::legacy_config_path();

    let primary_s = primary.to_string_lossy();
    let fallback_s = fallback.to_string_lossy();
    assert!(
        primary_s.contains(".poly"),
        "primary path must use .poly namespace"
    );
    assert!(
        !primary_s.contains(".resourcefully"),
        "primary path must NOT use .resourcefully namespace"
    );
    assert!(
        fallback_s.contains(".resourcefully"),
        "fallback path must use .resourcefully namespace"
    );
}
