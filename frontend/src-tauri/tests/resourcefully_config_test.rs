use app_lib::knowledge_graph::config::KnowledgeGraphSelection;
use app_lib::resourcefully_config::config::{
    CustomOpenAIConfigFields, KnowledgeGraphProfileWithoutSecrets,
    KnowledgeGraphSettingsWithoutSecrets, PreferencesConfig, ResourcefullyConfig, SummaryConfig,
    TranscriptConfig,
};

// ─── full schema roundtrip test ────────────────────────────────────────────

#[test]
fn resourcefully_config_full_schema_roundtrip_without_raw_secrets() {
    let cfg = ResourcefullyConfig {
        summary: SummaryConfig {
            provider: "openai".to_string(),
            model: "gpt-4o-2024-11-20".to_string(),
            whisper_model: "large-v3-turbo".to_string(),
            ollama_endpoint: Some("http://localhost:11434".to_string()),
        },
        transcript: TranscriptConfig {
            provider: "parakeet".to_string(),
            model: "parakeet-tdt-0.6b-v3-int8".to_string(),
        },
        custom_openai: CustomOpenAIConfigFields {
            endpoint: "https://api.custom-ai.example.com/v1".to_string(),
            model: "custom-model-v2".to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            top_p: Some(0.95),
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
    assert!(yaml.contains("large-v3-turbo"));
    assert!(yaml.contains("http://localhost:11434"));
    assert!(yaml.contains("parakeet"));
    assert!(yaml.contains("custom-model-v2"));
    assert!(yaml.contains("https://api.custom-ai.example.com/v1"));
    assert!(yaml.contains("max_tokens: 4096"));
    assert!(yaml.contains("temperature: 0.7"));
    assert!(yaml.contains("top_p: 0.95"));
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
    let restored: ResourcefullyConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(restored.summary.provider, cfg.summary.provider);
    assert_eq!(restored.summary.model, cfg.summary.model);
    assert_eq!(restored.summary.whisper_model, cfg.summary.whisper_model);
    assert_eq!(
        restored.summary.ollama_endpoint,
        cfg.summary.ollama_endpoint
    );
    assert_eq!(restored.transcript.provider, cfg.transcript.provider);
    assert_eq!(restored.transcript.model, cfg.transcript.model);
    assert_eq!(restored.custom_openai.endpoint, cfg.custom_openai.endpoint);
    assert_eq!(restored.custom_openai.model, cfg.custom_openai.model);
    assert_eq!(
        restored.custom_openai.max_tokens,
        cfg.custom_openai.max_tokens
    );
    assert_eq!(
        restored.custom_openai.temperature,
        cfg.custom_openai.temperature
    );
    assert_eq!(restored.custom_openai.top_p, cfg.custom_openai.top_p);
    assert_eq!(restored.knowledge_graph.profiles.len(), 1);
    assert_eq!(restored.knowledge_graph.profiles[0].id, "kg-1");
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
  provider: claude
  model: claude-3-opus
transcript:
  provider: groq
preferences:
  language: fr
"#;

    let cfg: ResourcefullyConfig = serde_yaml::from_str(partial_yaml).unwrap();

    // Set fields
    assert_eq!(cfg.summary.provider, "claude");
    assert_eq!(cfg.summary.model, "claude-3-opus");
    assert_eq!(cfg.transcript.provider, "groq");

    // Defaulted fields
    assert_eq!(cfg.summary.whisper_model, "large-v3-turbo"); // default
    assert_eq!(cfg.summary.ollama_endpoint, None); // default
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8"); // default
    assert_eq!(cfg.custom_openai.endpoint, ""); // default
    assert_eq!(cfg.custom_openai.model, ""); // default
    assert_eq!(cfg.custom_openai.max_tokens, None); // default
    assert_eq!(cfg.custom_openai.temperature, None); // default
    assert_eq!(cfg.custom_openai.top_p, None); // default
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
    let cfg: ResourcefullyConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(cfg.summary.provider, "openai");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(cfg.summary.whisper_model, "large-v3-turbo");
    assert_eq!(cfg.summary.ollama_endpoint, None);
    assert_eq!(cfg.transcript.provider, "parakeet");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(cfg.custom_openai.endpoint, "");
    assert_eq!(cfg.custom_openai.model, "");
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
    let result = ResourcefullyConfig::load_from_str(&contents);

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

    let cfg = ResourcefullyConfig {
        summary: SummaryConfig {
            provider: "ollama".to_string(),
            model: "llama3.1:8b".to_string(),
            whisper_model: "medium".to_string(),
            ollama_endpoint: Some("http://192.168.1.100:11434".to_string()),
        },
        transcript: TranscriptConfig {
            provider: "localWhisper".to_string(),
            model: "large-v3".to_string(),
        },
        custom_openai: CustomOpenAIConfigFields {
            endpoint: "http://localhost:8000/v1".to_string(),
            model: "mixtral-8x7b".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.3),
            top_p: None,
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
    let restored: ResourcefullyConfig = serde_yaml::from_str(&read_back).unwrap();

    assert_eq!(restored.summary.provider, "ollama");
    assert_eq!(restored.summary.model, "llama3.1:8b");
    assert_eq!(restored.summary.whisper_model, "medium");
    assert_eq!(
        restored.summary.ollama_endpoint.as_deref(),
        Some("http://192.168.1.100:11434")
    );
    assert_eq!(restored.transcript.provider, "localWhisper");
    assert_eq!(restored.transcript.model, "large-v3");
    assert_eq!(restored.custom_openai.endpoint, "http://localhost:8000/v1");
    assert_eq!(restored.custom_openai.model, "mixtral-8x7b");
    assert_eq!(restored.custom_openai.max_tokens, Some(2048));
    assert_eq!(restored.custom_openai.temperature, Some(0.3));
    assert_eq!(restored.custom_openai.top_p, None);
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
    let cfg = ResourcefullyConfig::load_default();
    assert_eq!(cfg.summary.provider, "openai");
    assert_eq!(cfg.preferences.language, "auto-translate");
}

#[test]
fn load_from_file_missing_returns_default() {
    let cfg = ResourcefullyConfig::load_from_path(&std::path::PathBuf::from(
        "/nonexistent/path/resourcefully.yml",
    ));
    assert!(cfg.is_ok(), "Missing file should return default, not error");
    let cfg = cfg.unwrap();
    assert_eq!(cfg.summary.provider, "openai");
}

// ─── Default implementations ───────────────────────────────────────────────

#[test]
fn default_resourcefully_config_has_sensible_values() {
    let cfg = ResourcefullyConfig::default();
    assert_eq!(cfg.summary.provider, "openai");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(cfg.summary.whisper_model, "large-v3-turbo");
    assert_eq!(cfg.transcript.provider, "parakeet");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(cfg.preferences.language, "auto-translate");
}

// ─── atomic save preserves existing file on failure ───────────────────────

#[test]
fn resourcefully_config_atomic_save_preserves_existing_file_on_failure() {
    use app_lib::resourcefully_config::repository::ConfigRepository;
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("config.yml");
    let repo = ConfigRepository::with_path(file_path.clone());

    // Write an original config file.
    let original_cfg = ResourcefullyConfig {
        summary: SummaryConfig {
            provider: "openai".to_string(),
            model: "original-model".to_string(),
            whisper_model: "large-v3-turbo".to_string(),
            ollama_endpoint: None,
        },
        transcript: TranscriptConfig::default(),
        custom_openai: CustomOpenAIConfigFields::default(),
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
    let new_cfg = ResourcefullyConfig {
        summary: SummaryConfig {
            provider: "claude".to_string(),
            model: "new-model".to_string(),
            whisper_model: "large-v3-turbo".to_string(),
            ollama_endpoint: None,
        },
        transcript: TranscriptConfig::default(),
        custom_openai: CustomOpenAIConfigFields::default(),
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
fn startup_creates_default_resourcefully_yaml_without_config_tables() {
    use app_lib::resourcefully_config::repository::ConfigRepository;
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
    assert_eq!(cfg.summary.provider, "openai");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(cfg.summary.whisper_model, "large-v3-turbo");
    assert_eq!(cfg.transcript.provider, "parakeet");
    assert_eq!(cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(cfg.preferences.language, "auto-translate");

    // Verify actual file contents
    let contents = fs::read_to_string(&file_path).unwrap();
    assert!(
        contents.contains("openai"),
        "YAML must contain default summary provider"
    );
    assert!(
        contents.contains("parakeet"),
        "YAML must contain transcript provider"
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
        cfg2.summary.provider, cfg.summary.provider,
        "Reloaded config must match original"
    );
    assert_eq!(cfg2.transcript.provider, cfg.transcript.provider);
}
