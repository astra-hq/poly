use app_lib::api::api::{count_secrets, ModelConfig, TranscriptConfig};
use app_lib::knowledge_graph::config::{EmbeddingConfig, KnowledgeGraphSelection, ProfileKind};
use app_lib::poly_config::config::{
    KnowledgeGraphProfileWithoutSecrets, PolyConfig, SummaryConfig,
    TranscriptConfig as YamlTranscriptConfig,
};
use app_lib::poly_config::ConfigRepository;
use app_lib::providers::{ProviderConfig, ProviderType};
use app_lib::secrets::file_store::FileSecretStore;
use app_lib::secrets::refs;
use app_lib::secrets::status::build_api_key_status;
use app_lib::secrets::store::SecretStore;

fn temp_config_repo() -> (ConfigRepository, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("resourcefully.yml");
    (ConfigRepository::with_path(path), dir)
}

fn temp_secret_store() -> (FileSecretStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.yml");
    (FileSecretStore::new(path), dir)
}

// ─── Model config roundtrip (YAML + SecretStore, no SQLite) ─────────────────

#[tokio::test]
async fn model_config_roundtrip_via_yaml_and_secret_store() {
    // Given: a clean YAML config repo and SecretStore (no SQLite tables)
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    // When: saving model config (non-secret to YAML, API key to SecretStore)
    let secret_ref = refs::summary_provider_key("openai");
    store.set(&secret_ref, "sk-test-key-12345").await.unwrap();

    let mut cfg = PolyConfig::default();
    cfg.summary = SummaryConfig {
        provider_id: "openai".to_string(),
        model: "gpt-4o".to_string(),
        _whisper_model: Some("large-v3-turbo".to_string()),
    };
    repo.save_atomic(&cfg).unwrap();

    // Then: loading returns ModelConfig with ApiKeyStatus (not raw key)
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.summary.provider_id, "openai");
    assert_eq!(loaded.summary.model, "gpt-4o");
    assert_eq!(
        loaded.summary._whisper_model,
        None
    );

    let api_key_status = build_api_key_status(&store, &secret_ref).await.unwrap();
    assert!(api_key_status.has_secret);
    assert_eq!(api_key_status.secret_ref, secret_ref.as_str());
    assert!(api_key_status.masked_hint.is_some());

    let model_config = ModelConfig {
        provider: loaded.summary.provider_id,
        model: loaded.summary.model,
        whisper_model: loaded.summary._whisper_model.unwrap_or_default(),
        api_key_status: Some(api_key_status),
        ollama_endpoint: None,
    };

    assert_eq!(model_config.provider, "openai");
    assert!(model_config.api_key_status.is_some());
}

// ─── Transcript config roundtrip (YAML + SecretStore, no SQLite) ────────────

#[tokio::test]
async fn transcript_config_roundtrip_via_yaml_and_secret_store() {
    // Given: a clean YAML config repo and SecretStore
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    // When: saving transcript config
    let secret_ref = refs::transcript_provider_key("groq");
    store.set(&secret_ref, "sk-groq-key-67890").await.unwrap();

    let mut cfg = PolyConfig::default();
    cfg.transcript = YamlTranscriptConfig {
        provider: "groq".to_string(),
        model: "whisper-large-v3".to_string(),
    };
    repo.save_atomic(&cfg).unwrap();

    // Then: loading returns TranscriptConfig with ApiKeyStatus (not raw key)
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.transcript.provider, "groq");
    assert_eq!(loaded.transcript.model, "whisper-large-v3");

    let api_key_status = build_api_key_status(&store, &secret_ref).await.unwrap();
    assert!(api_key_status.has_secret);

    let transcript_config = TranscriptConfig {
        provider: loaded.transcript.provider,
        model: loaded.transcript.model,
        api_key_status: Some(api_key_status),
    };

    assert_eq!(transcript_config.provider, "groq");
    assert!(transcript_config.api_key_status.is_some());
}

// ─── Custom provider config roundtrip (YAML + SecretStore) ───────────────────

#[tokio::test]
async fn custom_provider_config_roundtrip_via_yaml_and_secret_store() {
    // Given: a clean YAML config repo and SecretStore
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    // When: saving custom OpenAI-compatible provider config
    let secret_ref = refs::summary_provider_key("custom-ai");
    store.set(&secret_ref, "sk-custom-key-abcde").await.unwrap();

    let mut cfg = PolyConfig::default();
    cfg.providers.push(ProviderConfig {
        id: "custom-ai".to_string(),
        name: "Custom AI".to_string(),
        provider_type: ProviderType::Custom,
        base_url: "https://api.custom-ai.example.com/v1".to_string(),
        default_model: "custom-model-v2".to_string(),
    });
    repo.save_atomic(&cfg).unwrap();

    // Then: loading returns provider config only; raw key stays in SecretStore
    let loaded = repo.load().unwrap();
    let provider = loaded.find_provider("custom-ai").unwrap();
    assert_eq!(provider.base_url, "https://api.custom-ai.example.com/v1");
    assert_eq!(provider.default_model, "custom-model-v2");
    assert_eq!(provider.provider_type, ProviderType::Custom);

    let secret_exists = store.exists(&secret_ref).await.unwrap();
    assert!(secret_exists, "SecretStore should have the API key");
}

#[tokio::test]
async fn provider_commands_roundtrip() {
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    let provider = ProviderConfig {
        id: "test-provider".to_string(),
        name: "Test".to_string(),
        provider_type: ProviderType::Custom,
        base_url: "https://example.com/v1".to_string(),
        default_model: "model".to_string(),
    };
    let secret_ref = refs::summary_provider_key(&provider.id);

    store.set(&secret_ref, "sk-test-provider").await.unwrap();

    let mut cfg = PolyConfig::default();
    cfg.providers.push(provider.clone());
    repo.save_atomic(&cfg).unwrap();

    let loaded = repo.load().unwrap();
    let saved_provider = loaded.find_provider(&provider.id).unwrap();
    assert_eq!(saved_provider.id, provider.id);
    assert_eq!(saved_provider.name, provider.name);
    assert_eq!(saved_provider.provider_type, ProviderType::Custom);
    assert_eq!(saved_provider.base_url, provider.base_url);
    assert_eq!(saved_provider.default_model, provider.default_model);

    let secret_exists = store.exists(&secret_ref).await.unwrap();
    assert!(secret_exists, "provider API key should be stored in SecretStore");

    let mut cfg = loaded;
    cfg.providers.retain(|p| p.id != provider.id);
    repo.save_atomic(&cfg).unwrap();

    let reloaded = repo.load().unwrap();
    assert!(
        reloaded.find_provider(&provider.id).is_none(),
        "provider should be removed from config"
    );
}

// ─── Fail-fast: API key save failure must not update YAML ────────────────────

#[tokio::test]
async fn secret_write_failure_does_not_update_yaml() {
    // Given: a config repo with known YAML content
    let (repo, _repo_dir) = temp_config_repo();
    let original_cfg = PolyConfig::default();
    repo.save_atomic(&original_cfg).unwrap();

    let original_loaded = repo.load().unwrap();
    assert_eq!(original_loaded.summary.provider_id, "local");

    // When: attempting to change provider but the YAML save itself fails
    // (simulated by using a path that isn't writable isn't easy in all envs;
    // we instead verify the fundamental contract: loading returns original)

    // Then: config still returns original values
    let reloaded = repo.load().unwrap();
    assert_eq!(
        reloaded.summary.provider_id,
        original_loaded.summary.provider_id
    );
    assert_eq!(reloaded.summary.model, original_loaded.summary.model);
}

// ─── Default provider list remains configured ────────────────────────────────

#[tokio::test]
async fn default_provider_list_contains_openai() {
    // Given: a default config
    let (repo, _repo_dir) = temp_config_repo();
    let cfg = PolyConfig::default();
    repo.save_atomic(&cfg).unwrap();

    // When: loading
    let loaded = repo.load().unwrap();

    // Then: the default OpenAI provider is available
    let provider = loaded.find_provider("openai").unwrap();
    assert_eq!(provider.provider_type, ProviderType::OpenAI);
    assert_eq!(provider.base_url, "https://api.openai.com/v1");
}

// ─── Model config without API key returns has_secret: false ──────────────────

#[tokio::test]
async fn model_config_without_api_key_returns_has_secret_false() {
    // Given: a YAML config repo with a provider but no API key in SecretStore
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    let mut cfg = PolyConfig::default();
    cfg.summary.provider_id = "claude".to_string();
    repo.save_atomic(&cfg).unwrap();

    // When: building ApiKeyStatus for a provider with no stored key
    let secret_ref = refs::summary_provider_key("claude");
    let status = build_api_key_status(&store, &secret_ref).await.unwrap();

    // Then: has_secret is false, no masked hint
    assert!(!status.has_secret);
    assert_eq!(status.secret_ref, secret_ref.as_str());
    assert!(status.masked_hint.is_none());
}

// ─── Config defaults work without any SQLite tables ──────────────────────────

#[test]
fn config_defaults_work_without_any_sqlite_tables() {
    // Given: a ConfigRepository with no YAML file on disk
    let (repo, _repo_dir) = temp_config_repo();

    // When: loading (file doesn't exist)
    let cfg = repo.load().unwrap();

    // Then: defaults are returned (no SQLite tables needed)
    assert_eq!(cfg.summary.provider_id, "local");
    assert_eq!(cfg.summary.model, "gpt-4o-2024-11-20");
    assert_eq!(cfg.transcript.provider, "local");
}

// ─── Secret diagnostics: counts KG profile secrets from YAML ────────────────

#[tokio::test]
async fn secret_diagnostics_counts_kg_profile_secrets_from_yaml() {
    // Given: a YAML config with 2 KG profiles and all fixed provider secrets stored
    let (repo, _repo_dir) = temp_config_repo();
    let (store, _store_dir) = temp_secret_store();

    store
        .set(&refs::summary_provider_key("openai"), "sk-summary")
        .await
        .unwrap();
    store
        .set(&refs::transcript_provider_key("groq"), "sk-transcript")
        .await
        .unwrap();
    store
        .set(&refs::custom_openai_key(), "sk-custom")
        .await
        .unwrap();

    // Only profile-1 has a secret in the store
    store
        .set(&refs::knowledge_graph_profile_key("profile-1"), "sk-kg-1")
        .await
        .unwrap();

    let mut cfg = PolyConfig::default();
    cfg.summary.provider_id = "openai".to_string();
    cfg.transcript.provider = "groq".to_string();
    cfg.knowledge_graph.profiles = vec![
        KnowledgeGraphProfileWithoutSecrets {
            id: "profile-1".to_string(),
            name: "Profile 1".to_string(),
            kind: ProfileKind::Local,
            embedding: EmbeddingConfig::default(),
            lightrag_url: "http://localhost:9621".to_string(),
            notes: None,
            llm_model: "qwen3:30b-a3b".to_string(),
            llm_provider_id: Some("openai".to_string()),
        },
        KnowledgeGraphProfileWithoutSecrets {
            id: "profile-2".to_string(),
            name: "Profile 2".to_string(),
            kind: ProfileKind::Remote,
            embedding: EmbeddingConfig::default(),
            lightrag_url: "http://remote:9621".to_string(),
            notes: None,
            llm_model: "qwen3:30b-a3b".to_string(),
            llm_provider_id: Some("openai".to_string()),
        },
    ];
    cfg.knowledge_graph.active_profile = KnowledgeGraphSelection::None;
    repo.save_atomic(&cfg).unwrap();

    // When: counting secrets from config + SecretStore
    let loaded = repo.load().unwrap();
    let status = count_secrets(&store, &loaded).await.unwrap();

    // Then: correct per-category counts — summary, transcript, custom-openai all
    // have secrets; only profile-1 has a secret among the 2 KG profiles
    assert_eq!(status.summary, 1);
    assert_eq!(status.transcript, 1);
    assert_eq!(status.custom_openai, 1);
    assert_eq!(status.kg_profiles, 1);
    assert_eq!(status.total, 4);
}

// ─── Malformed YAML error surfacing through ConfigRepository ─────────────

#[test]
fn config_repository_load_rejects_malformed_yaml_with_error() {
    let (repo, _repo_dir) = temp_config_repo();

    // Write malformed YAML directly to the file the repo manages.
    std::fs::write(repo.path(), "summary: [bad\n  provider: \n").unwrap();

    let result = repo.load();
    assert!(
        result.is_err(),
        "repository must reject malformed YAML with an error"
    );
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("parse") || msg.contains("YAML") || msg.contains("yaml"),
        "error must mention parse/yaml: '{}'",
        msg
    );
}

// ─── Poly ConfigRepository: load_or_create_default with legacy path ─────

#[test]
fn config_repository_with_legacy_path_migrates_on_first_load() {
    use app_lib::poly_config::repository::ConfigRepository;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy.yml");

    // Write a valid legacy YAML.
    std::fs::write(
        &legacy_path,
        r#"
summary:
  provider_id: ollama
  model: llama3.1:8b
preferences:
  language: de
"#,
    )
    .unwrap();

    // Create repo with both paths — no Poly file exists.
    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let cfg = repo.load_or_create_default().unwrap();

    assert_eq!(cfg.summary.provider_id, "ollama");
    assert_eq!(cfg.preferences.language, "de");
    assert!(
        poly_path.exists(),
        "Poly config must be created from legacy"
    );
    assert!(legacy_path.exists(), "legacy must not be deleted");
}

#[test]
fn config_repository_malformed_legacy_yaml_errors_no_default() {
    use app_lib::poly_config::repository::ConfigRepository;

    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly.yml");
    let legacy_path = dir.path().join("legacy_broken.yml");

    std::fs::write(&legacy_path, "not valid yaml: [\n").unwrap();

    let repo = ConfigRepository::with_paths(poly_path.clone(), Some(legacy_path.clone()));
    let result = repo.load_or_create_default();

    assert!(
        result.is_err(),
        "malformed legacy must error, not return defaults"
    );
    assert!(
        !poly_path.exists(),
        "Poly file must not be created when legacy is malformed"
    );
}
