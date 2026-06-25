/// Integration tests for the secret migration workflow.
///
/// These tests exercise `SecretMigration::run` against an in-memory
/// SQLite database, verifying that secrets are migrated from legacy
/// config tables and the original columns are scrubbed.
use app_lib::secrets::file_store::FileSecretStore;
use app_lib::secrets::migration::{MigrationReport, SecretMigration};
use app_lib::secrets::refs;
use app_lib::secrets::store::SecretStore;
use sqlx::SqlitePool;

async fn init_db(pool: &SqlitePool) {
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
    .execute(pool)
    .await
    .unwrap();

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
    .execute(pool)
    .await
    .unwrap();
}

fn temp_store() -> (FileSecretStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSecretStore::new(dir.path().join("secrets.yml"));
    (store, dir)
}

// ── Happy path ──────────────────────────────────────────────────────

#[tokio::test]
async fn migration_on_empty_db_is_noop() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let (store, _dir) = temp_store();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 0);
    assert_eq!(report.scrubbed, 0);
}

#[tokio::test]
async fn migration_moves_single_openai_key() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey) VALUES ('1', 'openai', 'gpt-4', 'large-v3', 'sk-test')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 1);

    let key = refs::summary_provider_key("openai");
    assert_eq!(
        store.get(&key).await.unwrap().as_deref(),
        Some("sk-test")
    );
}

#[tokio::test]
async fn migration_moves_multiple_settings_keys() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey, groqApiKey, anthropicApiKey) VALUES ('1', 'openai', 'gpt-4', 'large-v3', 'openai-secret', 'groq-secret', 'anthropic-secret')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 3);

    assert_eq!(
        store
            .get(&refs::summary_provider_key("openai"))
            .await
            .unwrap()
            .as_deref(),
        Some("openai-secret")
    );
    assert_eq!(
        store
            .get(&refs::summary_provider_key("groq"))
            .await
            .unwrap()
            .as_deref(),
        Some("groq-secret")
    );
    assert_eq!(
        store
            .get(&refs::summary_provider_key("anthropic"))
            .await
            .unwrap()
            .as_deref(),
        Some("anthropic-secret")
    );
}

#[tokio::test]
async fn migration_moves_transcript_settings_keys() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO transcript_settings (id, provider, model, whisperApiKey, deepgramApiKey) VALUES ('1', 'whisper', 'large-v3', 'sk-whisper', 'sk-deepgram')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert!(report.migrated >= 2);

    assert_eq!(
        store
            .get(&refs::transcript_provider_key("whisper"))
            .await
            .unwrap()
            .as_deref(),
        Some("sk-whisper")
    );
    assert_eq!(
        store
            .get(&refs::transcript_provider_key("deepgram"))
            .await
            .unwrap()
            .as_deref(),
        Some("sk-deepgram")
    );
}

#[tokio::test]
async fn migration_moves_custom_openai_api_key() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    let config = serde_json::json!({
        "apiKey": "sk-custom-openai-key",
        "endpoint": "https://api.custom.com/v1"
    });
    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, customOpenAIConfig) VALUES ('1', 'openai', 'gpt-4', 'large-v3', $1)",
    )
    .bind(serde_json::to_string(&config).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 1);

    assert_eq!(
        store
            .get(&refs::custom_openai_key())
            .await
            .unwrap()
            .as_deref(),
        Some("sk-custom-openai-key")
    );
}

// ── Scrubbing ───────────────────────────────────────────────────────

#[tokio::test]
async fn migration_scrubs_settings_columns() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey) VALUES ('1', 'openai', 'gpt-4', 'large-v3', 'sk-will-be-scrubbed')",
    )
    .execute(&pool)
    .await
    .unwrap();

    SecretMigration::run(&pool, &store).await.unwrap();

    let (val,): (Option<String>,) =
        sqlx::query_as("SELECT openaiApiKey FROM settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(val.is_none(), "openaiApiKey should be NULL after scrub");
}

#[tokio::test]
async fn migration_scrubs_transcript_settings_columns() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO transcript_settings (id, provider, model, whisperApiKey) VALUES ('1', 'whisper', 'large-v3', 'sk-scrub-me')",
    )
    .execute(&pool)
    .await
    .unwrap();

    SecretMigration::run(&pool, &store).await.unwrap();

    let (val,): (Option<String>,) =
        sqlx::query_as("SELECT whisperApiKey FROM transcript_settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(val.is_none(), "whisperApiKey should be NULL after scrub");
}

// ── Idempotency ─────────────────────────────────────────────────────

#[tokio::test]
async fn migration_is_idempotent() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey) VALUES ('1', 'openai', 'gpt-4', 'large-v3', 'sk-idempotent')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let r1 = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(r1.migrated, 1, "first run must migrate 1");

    let r2 = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(r2.migrated, 0, "second run must migrate 0");

    // The value is still in the store.
    let key = refs::summary_provider_key("openai");
    assert_eq!(
        store.get(&key).await.unwrap().as_deref(),
        Some("sk-idempotent")
    );
}

// ── Edge cases ──────────────────────────────────────────────────────

#[tokio::test]
async fn migration_skips_empty_api_keys() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey, groqApiKey) VALUES ('1', 'openai', 'gpt-4', 'large-v3', '', '   ')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 0, "empty/whitespace keys must be skipped");
}

#[tokio::test]
async fn migration_skips_null_api_keys() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    // All API key columns default to NULL
    sqlx::query(
        "INSERT INTO settings (id, provider, model, whisperModel) VALUES ('1', 'openai', 'gpt-4', 'large-v3')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 0, "all-NULL row must produce zero migrations");
}

#[tokio::test]
async fn migration_with_no_settings_row_is_noop() {
    // Tables exist but have no data.
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    init_db(&pool).await;
    let (store, _dir) = temp_store();

    let report = SecretMigration::run(&pool, &store).await.unwrap();
    assert_eq!(report.migrated, 0);
    assert_eq!(report.scrubbed, 0);
}

// ── Report structure ────────────────────────────────────────────────

#[test]
fn migration_report_defaults_are_zero() {
    let report = MigrationReport::default();
    assert_eq!(report.migrated, 0);
    assert_eq!(report.scrubbed, 0);
}

#[test]
fn migration_report_equality() {
    let a = MigrationReport {
        migrated: 3,
        scrubbed: 1,
    };
    let b = MigrationReport {
        migrated: 3,
        scrubbed: 1,
    };
    assert_eq!(a, b);
}
