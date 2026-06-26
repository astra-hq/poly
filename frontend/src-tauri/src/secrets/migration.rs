use log::info;
use sqlx::Row;
use sqlx::SqlitePool;

use super::refs;
use super::store::SecretStore;
use super::types::SecretStoreError;

/// Migrates legacy secrets from SQLite config tables into a `SecretStore`.
///
/// After migration, the original SQLite columns are scrubbed (set to `NULL`)
/// so raw secrets no longer live in the database.
///
/// # Tables and columns scanned
///
/// | Table              | Column(s)                                                                 |
/// |--------------------|---------------------------------------------------------------------------|
/// | `settings`         | `groqApiKey`, `openaiApiKey`, `anthropicApiKey`, `ollamaApiKey`,          |
/// |                    | `openRouterApiKey`, `customOpenAIConfig` (JSON → `api_key` field)         |
/// | `transcript_settings` | `whisperApiKey`, `deepgramApiKey`, `elevenLabsApiKey`,                  |
/// |                    | `groqApiKey`, `openaiApiKey`                                              |
/// | `knowledge_graph_settings` (in `settings` table) | Each profile's `api_key`          |
pub struct SecretMigration;

impl SecretMigration {
    /// Run the full migration: scan → write → scrub.
    ///
    /// This is idempotent: already-migrated secrets (NULL columns) are
    /// harmless no-ops.
    pub async fn run(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
    ) -> Result<MigrationReport, SecretStoreError> {
        let mut report = MigrationReport::default();

        Self::migrate_settings(pool, store, &mut report).await?;
        Self::migrate_transcript_settings(pool, store, &mut report).await?;
        Self::migrate_knowledge_graph(pool, store, &mut report).await?;

        Ok(report)
    }

    /// Migrate secrets from the `settings` table.
    async fn migrate_settings(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
        report: &mut MigrationReport,
    ) -> Result<(), SecretStoreError> {
        let row = sqlx::query("SELECT * FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                // Treat "no such table" as empty — the table may not exist yet.
                if e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .map_err(|e| {
                SecretStoreError::StoreError(format!("failed to read settings table: {}", e))
            })?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        let migrations: Vec<(&str, Option<String>)> = vec![
            ("groq", read_column(&row, "groqApiKey")),
            ("openai", read_column(&row, "openaiApiKey")),
            ("anthropic", read_column(&row, "anthropicApiKey")),
            ("ollama", read_column(&row, "ollamaApiKey")),
            ("openRouter", read_column(&row, "openRouterApiKey")),
        ];

        // Migrate individual API keys
        for (provider, secret) in &migrations {
            if let Some(val) = secret {
                if val.trim().is_empty() {
                    continue;
                }
                let key = refs::summary_provider_key(provider);
                store.set(&key, val).await?;
                report.migrated += 1;
                info!("Migrated summary provider key: {}", key.as_str());
            }
        }

        // Migrate custom OpenAI config's api_key
        if let Some(json) = read_column(&row, "customOpenAIConfig") {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(api_key) = config.get("apiKey").or_else(|| config.get("api_key")) {
                    if let Some(val) = api_key.as_str() {
                        if !val.trim().is_empty() {
                            let key = refs::custom_openai_key();
                            store.set(&key, val).await?;
                            report.migrated += 1;
                            info!("Migrated custom OpenAI API key");
                        }
                    }
                }
            }
        }

        // Scrub the columns
        let scrub_sql = "
            UPDATE settings SET
                groqApiKey = NULL,
                openaiApiKey = NULL,
                anthropicApiKey = NULL,
                ollamaApiKey = NULL,
                openRouterApiKey = NULL,
                customOpenAIConfig = NULL
            WHERE id = (SELECT id FROM settings LIMIT 1)
        ";
        sqlx::query(scrub_sql).execute(pool).await.map_err(|e| {
            SecretStoreError::StoreError(format!("failed to scrub settings table: {}", e))
        })?;
        report.scrubbed += 1;

        Ok(())
    }

    /// Migrate secrets from the `transcript_settings` table.
    async fn migrate_transcript_settings(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
        report: &mut MigrationReport,
    ) -> Result<(), SecretStoreError> {
        let row = sqlx::query("SELECT * FROM transcript_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .map_err(|e| {
                SecretStoreError::StoreError(format!(
                    "failed to read transcript_settings table: {}",
                    e
                ))
            })?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        let migrations: Vec<(&str, Option<String>)> = vec![
            ("whisper", read_column(&row, "whisperApiKey")),
            ("deepgram", read_column(&row, "deepgramApiKey")),
            ("elevenLabs", read_column(&row, "elevenLabsApiKey")),
            ("groq", read_column(&row, "groqApiKey")),
            ("openai", read_column(&row, "openaiApiKey")),
        ];

        for (provider, secret) in &migrations {
            if let Some(val) = secret {
                if val.trim().is_empty() {
                    continue;
                }
                let key = refs::transcript_provider_key(provider);
                store.set(&key, val).await?;
                report.migrated += 1;
                info!("Migrated transcript provider key: {}", key.as_str());
            }
        }

        // Scrub
        let scrub_sql = "
            UPDATE transcript_settings SET
                whisperApiKey = NULL,
                deepgramApiKey = NULL,
                elevenLabsApiKey = NULL,
                groqApiKey = NULL,
                openaiApiKey = NULL
            WHERE id = (SELECT id FROM transcript_settings LIMIT 1)
        ";
        sqlx::query(scrub_sql).execute(pool).await.map_err(|e| {
            SecretStoreError::StoreError(format!(
                "failed to scrub transcript_settings table: {}",
                e
            ))
        })?;
        report.scrubbed += 1;

        Ok(())
    }

    /// Migrate knowledge graph API keys (embedded in JSON).
    async fn migrate_knowledge_graph(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
        report: &mut MigrationReport,
    ) -> Result<(), SecretStoreError> {
        let row = sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .map_err(|e| {
                SecretStoreError::StoreError(format!(
                    "failed to read knowledge_graph_settings: {}",
                    e
                ))
            })?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        let kg_json: Option<String> = row.try_get("knowledge_graph_settings").ok().flatten();

        let kg_json = match kg_json {
            Some(j) => j,
            None => return Ok(()),
        };

        let parsed: serde_json::Value = serde_json::from_str(&kg_json).unwrap_or_default();

        if let Some(profiles) = parsed.get("profiles").and_then(|p| p.as_array()) {
            for profile in profiles {
                if let Some(api_key) = profile
                    .get("api_key")
                    .or_else(|| profile.get("apiKey"))
                    .and_then(|v| v.as_str())
                {
                    if !api_key.trim().is_empty() {
                        let key = refs::knowledge_graph_key();
                        store.set(&key, api_key).await?;
                        report.migrated += 1;
                        info!("Migrated knowledge graph API key");
                        break; // Only one KG key tracked at root level
                    }
                }
            }
        }

        // Scrub by setting knowledge_graph_settings to NULL
        sqlx::query(
            "UPDATE settings SET knowledge_graph_settings = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        )
        .execute(pool)
        .await
        .map_err(|e| {
            SecretStoreError::StoreError(format!(
                "failed to scrub knowledge_graph_settings: {}",
                e
            ))
        })?;

        Ok(())
    }
}

/// Summary of a migration run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Number of individual secrets migrated.
    pub migrated: usize,
    /// Number of tables whose secret columns were scrubbed.
    pub scrubbed: usize,
}

/// Helper: extract a column value as `Option<String>` from a dynamic row.
fn read_column(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::file_store::FileSecretStore;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        // Create settings table
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

    #[tokio::test]
    async fn migrate_empty_db_is_noop() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(report.migrated, 0);
        assert_eq!(report.scrubbed, 0);
    }

    #[tokio::test]
    async fn migrate_settings_api_keys() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        // Insert a settings row with secrets
        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey, groqApiKey)
            VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', 'sk-test-openai', 'sk-test-groq')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(report.migrated, 2);

        // Verify secrets in store
        let openai_key = refs::summary_provider_key("openai");
        assert_eq!(
            store.get(&openai_key).await.unwrap().as_deref(),
            Some("sk-test-openai")
        );

        let groq_key = refs::summary_provider_key("groq");
        assert_eq!(
            store.get(&groq_key).await.unwrap().as_deref(),
            Some("sk-test-groq")
        );

        // Verify scrubbed columns
        let row: (Option<String>,) = sqlx::query_as("SELECT openaiApiKey FROM settings LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(row.0.is_none());
    }

    #[tokio::test]
    async fn migrate_transcript_settings_api_keys() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        sqlx::query(
            r#"
            INSERT INTO transcript_settings (id, provider, model, whisperApiKey)
            VALUES ('1', 'whisper', 'large-v3', 'sk-whisper-key')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(report.migrated, 1);

        let key = refs::transcript_provider_key("whisper");
        assert_eq!(
            store.get(&key).await.unwrap().as_deref(),
            Some("sk-whisper-key")
        );
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey)
            VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', 'sk-first')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // First migration
        let r1 = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(r1.migrated, 1, "first run should migrate 1");

        // Second migration — columns are already NULL, no new secrets
        let r2 = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(r2.migrated, 0, "second run should migrate 0");

        let key = refs::summary_provider_key("openai");
        assert_eq!(store.get(&key).await.unwrap().as_deref(), Some("sk-first"));
    }

    #[tokio::test]
    async fn empty_api_keys_are_skipped() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey, groqApiKey)
            VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', '', '   ')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(
            report.migrated, 0,
            "empty/whitespace keys should be skipped"
        );
    }

    #[tokio::test]
    async fn migrate_custom_openai_api_key() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        let config = serde_json::json!({
            "apiKey": "sk-custom-openai",
            "endpoint": "https://custom.api.com/v1"
        });
        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, customOpenAIConfig)
            VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', $1)
            "#,
        )
        .bind(serde_json::to_string(&config).unwrap())
        .execute(&pool)
        .await
        .unwrap();

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(report.migrated, 1);

        let key = refs::custom_openai_key();
        assert_eq!(
            store.get(&key).await.unwrap().as_deref(),
            Some("sk-custom-openai")
        );
    }

    #[tokio::test]
    async fn migration_report_counts_are_accurate() {
        let pool = setup_test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("secrets.yml"));

        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, openaiApiKey, groqApiKey, anthropicApiKey)
            VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', 'k1', 'k2', 'k3')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = SecretMigration::run(&pool, &store).await.unwrap();
        assert_eq!(report.migrated, 3, "should migrate 3 non-empty keys");

        // scrubbed: settings (1) + transcript_settings (1, even if row absent?
        // Actually transcript_settings scrubs only if a row exists. With no row,
        // the scrubbing of transcript_settings never happens.)
        // settings.scrub is always called if a row exists for settings.
        assert_eq!(report.scrubbed, 1, "only settings has a row to scrub");
    }
}
