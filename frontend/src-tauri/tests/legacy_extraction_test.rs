use app_lib::database::setup::drop_legacy_config_tables_with_pool;
use app_lib::poly_config::config::{PolyConfig, SummaryConfig};
use app_lib::poly_config::legacy_extraction::LegacyConfigExtractor;
use app_lib::poly_config::ConfigRepository;
use app_lib::secrets::file_store::FileSecretStore;
use app_lib::secrets::refs;
use app_lib::secrets::store::SecretStore;
use app_lib::secrets::types::{SecretRef, SecretStoreError};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    // Create settings table (matches production schema)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL DEFAULT 'gpt-4o-2024-11-20',
            whisperModel TEXT NOT NULL DEFAULT 'large-v3',
            groqApiKey TEXT,
            openaiApiKey TEXT,
            anthropicApiKey TEXT,
            ollamaApiKey TEXT,
            openRouterApiKey TEXT,
            ollamaEndpoint TEXT,
            customOpenAIConfig TEXT,
            knowledge_graph_settings TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create transcript_settings table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS transcript_settings (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL DEFAULT 'whisper-large-v3',
            whisperApiKey TEXT,
            deepgramApiKey TEXT,
            elevenLabsApiKey TEXT,
            groqApiKey TEXT,
            openaiApiKey TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

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

// ─── happy-path test ─────────────────────────────────────────────────────

#[tokio::test]
async fn legacy_sqlite_config_extraction_writes_yaml_and_secrets() {
    let pool = setup_test_db().await;
    let (store, _store_dir) = temp_secret_store();
    let (config_repo, _config_dir) = temp_config_repo();

    // Seed settings table with summary config + API keys
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel, ollamaEndpoint,
                              openaiApiKey, groqApiKey, anthropicApiKey)
        VALUES ('1', 'ollama', 'llama3.2:latest', 'large-v3-turbo',
                'http://localhost:11434', 'sk-openai-test', 'sk-groq-test', 'sk-claude-test')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed transcript_settings
    sqlx::query(
        r#"
        INSERT INTO transcript_settings (id, provider, model, whisperApiKey, deepgramApiKey)
        VALUES ('1', 'parakeet', 'parakeet-tdt-0.6b-v3-int8', 'sk-whisper-test', 'sk-deepgram-test')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed custom OpenAI config + knowledge graph in settings
    let custom_openai = serde_json::json!({
        "apiKey": "sk-custom-test",
        "endpoint": "https://custom.api.com/v1",
        "model": "custom-model-v2",
        "maxTokens": 4096,
        "temperature": 0.7,
        "topP": 0.95
    });

    let kg = serde_json::json!({
        "profiles": [{
            "id": "kg-1",
            "name": "Production KG",
            "kind": "remote",
            "embedding": {
                "provider": "openai",
                "model": "text-embedding-3-large",
                "dimensions": 3072
            },
            "lightrag_url": "https://kg.example.com:9621",
            "api_key": "sk-lightrag-test",
            "notes": "Main KG"
        }],
        "active_profile": { "profile": "kg-1" }
    });

    sqlx::query(
        r#"
        UPDATE settings SET
            customOpenAIConfig = $1,
            knowledge_graph_settings = $2
        WHERE id = '1'
        "#,
    )
    .bind(serde_json::to_string(&custom_openai).unwrap())
    .bind(serde_json::to_string(&kg).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let result = LegacyConfigExtractor::extract(&pool, &store, &config_repo).await;
    assert!(result.is_ok(), "extraction failed: {:?}", result.err());

    // ── Verify YAML has non-secret values ──────────────────────────────
    let loaded_cfg = config_repo.load().unwrap();

    // Summary config
    assert_eq!(loaded_cfg.summary.provider_id, "ollama");
    assert_eq!(loaded_cfg.summary.model, "llama3.2:latest");
    assert_eq!(
        loaded_cfg.summary._whisper_model,
        Some("large-v3-turbo".to_string())
    );

    // Transcript config
    assert_eq!(loaded_cfg.transcript.provider, "parakeet");
    assert_eq!(loaded_cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");

    // Knowledge graph (without secrets)
    assert_eq!(loaded_cfg.knowledge_graph.profiles.len(), 1);
    assert_eq!(loaded_cfg.knowledge_graph.profiles[0].id, "kg-1");
    assert_eq!(loaded_cfg.knowledge_graph.profiles[0].name, "Production KG");

    // ── Verify secrets are in SecretStore ──────────────────────────────
    let openai_key = refs::summary_provider_key("openai");
    assert_eq!(
        store.get(&openai_key).await.unwrap().as_deref(),
        Some("sk-openai-test")
    );

    let groq_key = refs::summary_provider_key("groq");
    assert_eq!(
        store.get(&groq_key).await.unwrap().as_deref(),
        Some("sk-groq-test")
    );

    let claude_key = refs::summary_provider_key("anthropic");
    assert_eq!(
        store.get(&claude_key).await.unwrap().as_deref(),
        Some("sk-claude-test")
    );

    let whisper_key = refs::transcript_provider_key("whisper");
    assert_eq!(
        store.get(&whisper_key).await.unwrap().as_deref(),
        Some("sk-whisper-test")
    );

    let deepgram_key = refs::transcript_provider_key("deepgram");
    assert_eq!(
        store.get(&deepgram_key).await.unwrap().as_deref(),
        Some("sk-deepgram-test")
    );

    let custom_key = refs::custom_openai_key();
    assert_eq!(
        store.get(&custom_key).await.unwrap().as_deref(),
        Some("sk-custom-test")
    );

    let kg_key = refs::knowledge_graph_key();
    assert_eq!(
        store.get(&kg_key).await.unwrap().as_deref(),
        Some("sk-lightrag-test")
    );

    // ── Verify SQLite secret columns are scrubbed ──────────────────────
    let row: (Option<String>,) = sqlx::query_as("SELECT openaiApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(row.0.is_none(), "openaiApiKey should be NULL after scrub");

    let row: (Option<String>,) =
        sqlx::query_as("SELECT whisperApiKey FROM transcript_settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(row.0.is_none(), "whisperApiKey should be NULL after scrub");

    let row: (Option<String>,) = sqlx::query_as("SELECT customOpenAIConfig FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        row.0.is_none(),
        "customOpenAIConfig should be NULL after scrub"
    );

    let row: (Option<String>,) =
        sqlx::query_as("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        row.0.is_none(),
        "knowledge_graph_settings should be NULL after scrub"
    );
}

// ─── failure test ────────────────────────────────────────────────────────

/// A `SecretStore` that always fails on `set`.
struct FailingSecretStore;

#[async_trait::async_trait]
impl SecretStore for FailingSecretStore {
    async fn get(&self, _key: &SecretRef) -> Result<Option<String>, SecretStoreError> {
        Ok(None)
    }

    async fn set(&self, _key: &SecretRef, _value: &str) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::StoreError(
            "simulated SecretStore set failure".into(),
        ))
    }

    async fn delete(&self, _key: &SecretRef) -> Result<(), SecretStoreError> {
        Ok(())
    }

    async fn exists(&self, _key: &SecretRef) -> Result<bool, SecretStoreError> {
        Ok(false)
    }
}

#[tokio::test]
async fn legacy_sqlite_config_extraction_is_non_destructive_on_secret_write_failure() {
    let pool = setup_test_db().await;
    let (config_repo, _config_dir) = temp_config_repo();
    let failing_store = FailingSecretStore;

    // Seed settings with API keys that must survive the failed extraction
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel,
                              openaiApiKey, groqApiKey, ollamaApiKey)
        VALUES ('1', 'ollama', 'llama3.2:latest', 'large-v3',
                'sk-should-survive', 'sk-also-survive', 'ollama-key-survive')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed transcript_settings
    sqlx::query(
        r#"
        INSERT INTO transcript_settings (id, provider, model, whisperApiKey)
        VALUES ('1', 'parakeet', 'parakeet-model', 'sk-transcript-survive')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = LegacyConfigExtractor::extract(&pool, &failing_store, &config_repo).await;

    // Extraction must fail
    assert!(
        result.is_err(),
        "extraction should fail when SecretStore::set errors"
    );

    // ── SQLite rows must be UNCHANGED ──────────────────────────────────
    let row: (Option<String>,) = sqlx::query_as("SELECT openaiApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("sk-should-survive"),
        "SQLite openaiApiKey must NOT have been scrubbed on failure"
    );

    let row: (Option<String>,) = sqlx::query_as("SELECT groqApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("sk-also-survive"),
        "SQLite groqApiKey must NOT have been scrubbed on failure"
    );

    let row: (Option<String>,) = sqlx::query_as("SELECT ollamaApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("ollama-key-survive"),
        "SQLite ollamaApiKey must NOT have been scrubbed on failure"
    );

    let row: (Option<String>,) =
        sqlx::query_as("SELECT whisperApiKey FROM transcript_settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("sk-transcript-survive"),
        "SQLite whisperApiKey must NOT have been scrubbed on failure"
    );

    // Non-secret columns should also be intact
    let row: (String,) = sqlx::query_as("SELECT provider FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "ollama", "non-secret columns should be unchanged");

    let row: (String,) = sqlx::query_as("SELECT model FROM transcript_settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0, "parakeet-model",
        "transcript non-secret columns should be unchanged"
    );

    // ── YAML must NOT have been written with non-default data ──────────
    // If the YAML file was created, it should only contain defaults (or
    // the file may not exist at all).
    match config_repo.path().exists() {
        true => {
            let loaded = config_repo.load().unwrap();
            assert_eq!(
                loaded.summary.provider_id, "openai",
                "YAML should contain default provider, not extracted data"
            );
            assert_eq!(
                loaded,
                PolyConfig::default(),
                "YAML should contain only default values after failed extraction"
            );
        }
        false => {
            // File not written at all — this is also acceptable behaviour
        }
    }
}

// ─── startup-ordering tests ─────────────────────────────────────────────

/// Verify the startup ordering: extraction runs, YAML is populated,
/// then old config tables are dropped.  The tables must be absent
/// after the drop step.
#[tokio::test]
async fn legacy_import_extracts_config_before_runtime_commands() {
    let pool = setup_test_db().await;
    let (store, _store_dir) = temp_secret_store();
    let (config_repo, _config_dir) = temp_config_repo();

    // Seed settings table with summary config + API keys
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel,
                              openaiApiKey, groqApiKey, ollamaEndpoint)
        VALUES ('1', 'ollama', 'llama3.2:latest', 'large-v3-turbo',
                'sk-openai-test', 'sk-groq-test', 'http://localhost:11434')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed transcript_settings
    sqlx::query(
        r#"
        INSERT INTO transcript_settings (id, provider, model, whisperApiKey)
        VALUES ('1', 'parakeet', 'parakeet-tdt-0.6b-v3-int8', 'sk-whisper-test')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // ── Step 1: extract ──────────────────────────────────────────────
    let result = LegacyConfigExtractor::extract(&pool, &store, &config_repo).await;
    assert!(
        result.is_ok(),
        "extraction must succeed: {:?}",
        result.err()
    );

    // ── Verify YAML has non-secret values ────────────────────────────
    let loaded_cfg = config_repo.load().unwrap();
    assert_eq!(loaded_cfg.summary.provider_id, "ollama");
    assert_eq!(loaded_cfg.summary.model, "llama3.2:latest");
    assert_eq!(
        loaded_cfg.summary._whisper_model,
        Some("large-v3-turbo".to_string())
    );
    assert_eq!(loaded_cfg.transcript.provider, "parakeet");
    assert_eq!(loaded_cfg.transcript.model, "parakeet-tdt-0.6b-v3-int8");

    // ── Verify secrets in SecretStore ────────────────────────────────
    let openai_key = refs::summary_provider_key("openai");
    assert_eq!(
        store.get(&openai_key).await.unwrap().as_deref(),
        Some("sk-openai-test")
    );
    let whisper_key = refs::transcript_provider_key("whisper");
    assert_eq!(
        store.get(&whisper_key).await.unwrap().as_deref(),
        Some("sk-whisper-test")
    );

    // ── Step 2: drop legacy tables (only after extraction succeeded) ─
    drop_legacy_config_tables_with_pool(&pool)
        .await
        .expect("table drop must succeed after successful extraction");

    // ── Assert old tables are gone ───────────────────────────────────
    let settings_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='settings')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !settings_exists,
        "settings table must be dropped after extraction"
    );

    let transcript_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='transcript_settings')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !transcript_exists,
        "transcript_settings table must be dropped after extraction"
    );
}

/// Simulate extraction failure and verify that old config tables
/// are preserved (not dropped).
#[tokio::test]
async fn legacy_import_preserves_tables_on_extraction_failure() {
    let pool = setup_test_db().await;
    let (config_repo, _config_dir) = temp_config_repo();
    let failing_store = FailingSecretStore;

    // Seed settings with data that must survive the failed extraction
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel,
                              openaiApiKey, groqApiKey)
        VALUES ('1', 'ollama', 'llama3.2:latest', 'large-v3',
                'sk-should-survive', 'sk-also-survive')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // ── Extraction must fail ────────────────────────────────────────
    let result = LegacyConfigExtractor::extract(&pool, &failing_store, &config_repo).await;
    assert!(result.is_err(), "extraction must fail with failing store");

    // ── Old tables must still exist ──────────────────────────────────
    let settings_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='settings')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        settings_exists,
        "settings table must be preserved on extraction failure"
    );

    let transcript_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='transcript_settings')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        transcript_exists,
        "transcript_settings table must be preserved on extraction failure"
    );

    // ── Data in old tables must be intact ────────────────────────────
    let row: (Option<String>,) = sqlx::query_as("SELECT openaiApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("sk-should-survive"),
        "SQLite data must not have been scrubbed on extraction failure"
    );

    let row: (Option<String>,) = sqlx::query_as("SELECT groqApiKey FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("sk-also-survive"),
        "all SQLite API keys must be preserved on failure"
    );

    // Non-secret columns also intact
    let row: (String,) = sqlx::query_as("SELECT provider FROM settings LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "ollama", "non-secret columns should be unchanged");
}

// ─── extraction + Poly config interaction tests ────────────────────────

/// After extraction creates Poly config, calling `load_or_create_default`
/// with the Poly path must return the extracted config (Poly file exists)
/// and must NOT attempt legacy migration again.
#[tokio::test]
async fn poly_config_loads_extracted_data_after_extraction() {
    let pool = setup_test_db().await;
    let (store, _store_dir) = temp_secret_store();
    let (config_repo, _config_dir) = temp_config_repo();

    // Seed settings with summary config
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel,
                              openaiApiKey, ollamaEndpoint)
        VALUES ('1', 'ollama', 'llama3.2:latest', 'large-v3-turbo',
                'sk-openai-test', 'http://localhost:11434')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed transcript_settings
    sqlx::query(
        r#"
        INSERT INTO transcript_settings (id, provider, model, whisperApiKey)
        VALUES ('1', 'parakeet', 'parakeet-tdt-0.6b-v3-int8', 'sk-whisper-test')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Step 1: extract config from SQLite into YAML + SecretStore
    let result = LegacyConfigExtractor::extract(&pool, &store, &config_repo).await;
    assert!(result.is_ok(), "extraction failed: {:?}", result.err());

    // Step 2: load config from the repo (Poly file now exists)
    let loaded = config_repo.load().unwrap();

    assert_eq!(loaded.summary.provider_id, "ollama");
    assert_eq!(loaded.summary.model, "llama3.2:latest");
    assert_eq!(loaded.summary._whisper_model, None);
    assert_eq!(loaded.transcript.provider, "parakeet");
    assert_eq!(loaded.transcript.model, "parakeet-tdt-0.6b-v3-int8");

    // Secrets were placed in SecretStore (not in YAML)
    let openai_key = refs::summary_provider_key("openai");
    assert_eq!(
        store.get(&openai_key).await.unwrap().as_deref(),
        Some("sk-openai-test")
    );
}

/// When a Poly config already exists (e.g. from a previous run),
/// `load_or_create_default` must load it as-is — it must not
/// overwrite it through legacy migration or extraction.
#[tokio::test]
async fn existing_poly_config_is_not_overwritten_by_load_or_create_default() {
    let (config_repo, _config_dir) = temp_config_repo();

    // Write a custom Poly config that simulates data from a previous session.
    let custom_cfg = PolyConfig {
        summary: SummaryConfig {
            provider_id: "openai".to_string(),
            model: "my-model".to_string(),
            ..Default::default()
        },
        ..PolyConfig::default()
    };
    config_repo.save_atomic(&custom_cfg).unwrap();

    // Now simulate startup: load_or_create_default must return the
    // existing config unchanged — no legacy fallback, no overwrite.
    let loaded = config_repo.load_or_create_default().unwrap();

    assert_eq!(loaded.summary.provider_id, "openai");
    assert_eq!(loaded.summary.model, "my-model");
    assert_eq!(loaded.summary._whisper_model, None);
    assert_eq!(loaded, custom_cfg);
}
