//! Legacy SQLite → YAML + SecretStore extraction.
//!
//! `LegacyConfigExtractor` is the **only** runtime code allowed to read the
//! old `settings`, `transcript_settings`, and `knowledge_graph_settings`
//! tables.  It performs a one-shot migration that:
//!
//! 1. Scans the legacy SQLite tables for non-secret configuration values.
//! 2. Writes raw API keys to the [`SecretStore`] using canonical refs from
//!    [`crate::secrets::refs`].
//! 3. Persists the non-secret config as YAML via
//!    [`ConfigRepository::save_atomic`].
//! 4. Reads back the YAML and SecretStore entries to verify correctness.
//! 5. Scrubs SQLite secret columns (`SET … = NULL`) **only** after
//!    verification succeeds.
//!
//! # Failure behaviour
//!
//! - If Step 2 (SecretStore write) fails, Steps 3–5 are **never** executed
//!   and SQLite rows are left untouched.
//! - If Step 3 (YAML write) or Step 4 (verification) fails, Step 5 is
//!   skipped — SQLite data is preserved.
//! - Missing tables are handled gracefully (the extractor returns `Ok` with
//!   an empty/default config).

use anyhow::{anyhow, Context, Result};
use log::info;
use sqlx::Row;
use sqlx::SqlitePool;

use super::config::{
    KnowledgeGraphProfileWithoutSecrets, KnowledgeGraphSettingsWithoutSecrets, PolyConfig,
    PreferencesConfig, SummaryConfig, TranscriptConfig,
};
use super::ConfigRepository;
use crate::knowledge_graph::config::KnowledgeGraphSettings;
use crate::secrets::refs;
use crate::secrets::store::SecretStore;

/// The legacy-extraction boundary.
///
/// Call [`extract`](LegacyConfigExtractor::extract) once at first-run or
/// migration time.  This struct holds no state.
pub struct LegacyConfigExtractor;

impl LegacyConfigExtractor {
    /// Run the full extraction pipeline.
    ///
    /// The steps are sequential and atomic with respect to the caller:
    /// secret-write → YAML-write → verification → scrub.  Any failure
    /// before scrub leaves SQLite untouched.
    pub async fn extract(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
        config_repo: &ConfigRepository,
    ) -> Result<()> {
        // 1. Scan legacy tables for non-secret configuration.
        let cfg = Self::scan(pool).await?;

        // 2. Write API keys to SecretStore FIRST (fail-fast contract).
        Self::write_secrets(pool, store)
            .await
            .context("failed to write legacy secrets to SecretStore")?;

        // 3. Persist non-secret config as YAML.
        config_repo
            .save_atomic(&cfg)
            .context("failed to write non-secret config to YAML")?;

        // 4. Read back and verify both YAML and SecretStore.
        Self::verify(pool, store, config_repo, &cfg)
            .await
            .context("verification after extraction failed — SQLite left untouched")?;

        // 5. Scrub SQLite secret columns.
        Self::scrub(pool)
            .await
            .context("failed to scrub legacy SQLite secret columns")?;

        Ok(())
    }

    // ── scan ──────────────────────────────────────────────────────────────

    /// Scan all legacy SQLite tables and assemble a [`PolyConfig`]
    /// with every **non-secret** value.
    ///
    /// Handles missing tables gracefully — returns `PolyConfig::default()`.
    async fn scan(pool: &SqlitePool) -> Result<PolyConfig> {
        let summary = Self::scan_summary(pool).await.unwrap_or_default();
        let transcript = Self::scan_transcript(pool).await.unwrap_or_default();
        let knowledge_graph = Self::scan_knowledge_graph(pool).await.unwrap_or_default();

        Ok(PolyConfig {
            providers: vec![],
            summary,
            transcript,
            knowledge_graph,
            preferences: PreferencesConfig::default(),
            calendar: Default::default(),
        })
    }

    /// Scan `settings` table for summary config (non-secret fields).
    async fn scan_summary(pool: &SqlitePool) -> Result<SummaryConfig> {
        let row = sqlx::query(
            "SELECT provider, model, whisperModel, ollamaEndpoint FROM settings LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .or_else(|e| {
            if is_missing_table(&e) {
                Ok(None)
            } else {
                Err(e)
            }
        })
        .with_context(|| "failed to scan settings table")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(SummaryConfig::default()),
        };

        Ok(SummaryConfig {
            provider_id: read_column_string(&row, "provider").unwrap_or_else(|| "openai".into()),
            model: read_column_string(&row, "model").unwrap_or_else(|| "gpt-4o-2024-11-20".into()),
            ..Default::default()
        })
    }

    /// Scan `transcript_settings` table for transcript config (non-secret fields).
    async fn scan_transcript(pool: &SqlitePool) -> Result<TranscriptConfig> {
        let row = sqlx::query("SELECT provider, model FROM transcript_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to scan transcript_settings table")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(TranscriptConfig::default()),
        };

        Ok(TranscriptConfig {
            provider: read_column_string(&row, "provider").unwrap_or_else(|| "parakeet".into()),
            model: read_column_string(&row, "model")
                .unwrap_or_else(|| crate::config::DEFAULT_PARAKEET_MODEL.into()),
        })
    }

    /// Scan `settings.knowledge_graph_settings` JSON, stripping `api_key`.
    async fn scan_knowledge_graph(
        pool: &SqlitePool,
    ) -> Result<KnowledgeGraphSettingsWithoutSecrets> {
        let row = sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to scan knowledge_graph_settings column")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(KnowledgeGraphSettingsWithoutSecrets::default()),
        };

        let json: Option<String> = row.try_get("knowledge_graph_settings").ok().flatten();

        let json = match json {
            Some(j) => j,
            None => return Ok(KnowledgeGraphSettingsWithoutSecrets::default()),
        };

        let parsed: KnowledgeGraphSettings = serde_json::from_str(&json).unwrap_or_default();

        Ok(KnowledgeGraphSettingsWithoutSecrets {
            profiles: parsed
                .profiles
                .into_iter()
                .map(KnowledgeGraphProfileWithoutSecrets::from)
                .collect(),
            active_profile: parsed.active_profile,
        })
    }

    // ── write_secrets ─────────────────────────────────────────────────────

    /// Write all API keys from legacy SQLite columns into the [`SecretStore`].
    ///
    /// **If any write fails, the error propagates immediately** — no YAML
    /// will be written and SQLite will not be scrubbed.
    async fn write_secrets(pool: &SqlitePool, store: &(dyn SecretStore + Sync)) -> Result<()> {
        Self::write_summary_secrets(pool, store).await?;
        Self::write_transcript_secrets(pool, store).await?;
        Self::write_kg_secrets(pool, store).await?;
        Ok(())
    }

    async fn write_summary_secrets(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
    ) -> Result<()> {
        let row = sqlx::query("SELECT * FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to read settings table for secret extraction")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        // Summary API keys → summary_provider_key
        let migrations: Vec<(&str, Option<String>)> = vec![
            ("groq", read_column(&row, "groqApiKey")),
            ("openai", read_column(&row, "openaiApiKey")),
            ("anthropic", read_column(&row, "anthropicApiKey")),
            ("ollama", read_column(&row, "ollamaApiKey")),
            ("openRouter", read_column(&row, "openRouterApiKey")),
        ];

        for (provider, secret) in &migrations {
            if let Some(val) = secret {
                if val.trim().is_empty() {
                    continue;
                }
                let key = refs::summary_provider_key(provider);
                store.set(&key, val).await.map_err(|e| {
                    anyhow!("failed to write summary secret {}: {}", key.as_str(), e)
                })?;
                info!("extracted legacy summary secret: {}", key.as_str());
            }
        }

        // Custom OpenAI API key from JSON
        let custom_json: Option<String> = read_column(&row, "customOpenAIConfig");
        if let Some(json) = custom_json {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(api_key) = parsed
                    .get("apiKey")
                    .or_else(|| parsed.get("api_key"))
                    .and_then(|v| v.as_str())
                {
                    if !api_key.trim().is_empty() {
                        let key = refs::custom_openai_key();
                        store
                            .set(&key, api_key)
                            .await
                            .map_err(|e| anyhow!("failed to write custom OpenAI secret: {}", e))?;
                        info!("extracted legacy custom-openai secret");
                    }
                }
            }
        }

        Ok(())
    }

    async fn write_transcript_secrets(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
    ) -> Result<()> {
        let row = sqlx::query("SELECT * FROM transcript_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to read transcript_settings table for secret extraction")?;

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
                store.set(&key, val).await.map_err(|e| {
                    anyhow!("failed to write transcript secret {}: {}", key.as_str(), e)
                })?;
                info!("extracted legacy transcript secret: {}", key.as_str());
            }
        }

        Ok(())
    }

    async fn write_kg_secrets(pool: &SqlitePool, store: &(dyn SecretStore + Sync)) -> Result<()> {
        let row = sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to read knowledge_graph_settings for secret extraction")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        let json: Option<String> = row.try_get("knowledge_graph_settings").ok().flatten();

        let json = match json {
            Some(j) => j,
            None => return Ok(()),
        };

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();

        if let Some(profiles) = parsed.get("profiles").and_then(|p| p.as_array()) {
            for profile in profiles {
                if let Some(api_key) = profile
                    .get("api_key")
                    .or_else(|| profile.get("apiKey"))
                    .and_then(|v| v.as_str())
                {
                    if !api_key.trim().is_empty() {
                        let key = refs::knowledge_graph_key();
                        store.set(&key, api_key).await.map_err(|e| {
                            anyhow!("failed to write knowledge graph secret: {}", e)
                        })?;
                        info!("extracted legacy knowledge-graph secret");
                        break; // one KG key at root level
                    }
                }
            }
        }

        Ok(())
    }

    // ── verify ────────────────────────────────────────────────────────────

    /// Verify that both YAML and SecretStore contain the expected values.
    ///
    /// Reads the config file back via `config_repo.load()` and asserts
    /// structural equality against `expected`.  Then confirms that every
    /// secret we expected to write is actually present in the store.
    async fn verify(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
        config_repo: &ConfigRepository,
        expected: &PolyConfig,
    ) -> Result<()> {
        // Verify YAML config
        let loaded = config_repo
            .load()
            .with_context(|| "failed to read back YAML config for verification")?;

        if loaded != *expected {
            return Err(anyhow!(
                "YAML verification failed: loaded config differs from expected.\n\
                 expected: {:?}\nloaded: {:?}",
                expected,
                loaded
            ));
        }

        // Verify every secret we expect to find is in the store.
        Self::verify_summary_secrets(pool, store).await?;
        Self::verify_transcript_secrets(pool, store).await?;
        Self::verify_kg_secrets(pool, store).await?;

        Ok(())
    }

    async fn verify_summary_secrets(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
    ) -> Result<()> {
        let row = match Self::fetch_settings_row(pool).await? {
            Some(r) => r,
            None => return Ok(()),
        };

        let checks: Vec<(&str, &str)> = vec![
            ("groq", "groqApiKey"),
            ("openai", "openaiApiKey"),
            ("anthropic", "anthropicApiKey"),
            ("ollama", "ollamaApiKey"),
            ("openRouter", "openRouterApiKey"),
        ];

        for (provider, column) in &checks {
            let expected: Option<String> = read_column(&row, column);
            let key = refs::summary_provider_key(provider);

            // If the legacy column is empty/None, nothing to verify.
            let expected_val = match expected {
                Some(ref v) if !v.trim().is_empty() => v.clone(),
                _ => continue,
            };

            let stored = store
                .get(&key)
                .await
                .map_err(|e| anyhow!("verification read failed for {}: {}", key.as_str(), e))?;

            match stored {
                Some(ref s) if s == &expected_val => {
                    // Match — ok.
                }
                Some(s) => {
                    return Err(anyhow!(
                        "verification mismatch for {}: expected {:?}, stored {:?}",
                        key.as_str(),
                        expected_val,
                        s
                    ));
                }
                None => {
                    return Err(anyhow!(
                        "verification: secret {} was expected but not found in store",
                        key.as_str()
                    ));
                }
            }
        }

        // Verify custom OpenAI key from JSON
        let custom_json: Option<String> = read_column(&row, "customOpenAIConfig");
        if let Some(json) = custom_json {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(expected_key) = parsed
                    .get("apiKey")
                    .or_else(|| parsed.get("api_key"))
                    .and_then(|v| v.as_str())
                {
                    if !expected_key.trim().is_empty() {
                        let key = refs::custom_openai_key();
                        let stored = store.get(&key).await.map_err(|e| {
                            anyhow!("verification read failed for custom-openai: {}", e)
                        })?;
                        match stored {
                            None => {
                                return Err(anyhow!(
                                    "verification: custom-openai secret expected but not found"
                                ));
                            }
                            Some(s) if s != expected_key => {
                                return Err(anyhow!(
                                    "verification mismatch for custom-openai: expected {:?}, stored {:?}",
                                    expected_key,
                                    s
                                ));
                            }
                            Some(_) => { /* match */ }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn verify_transcript_secrets(
        pool: &SqlitePool,
        store: &(dyn SecretStore + Sync),
    ) -> Result<()> {
        let row = sqlx::query("SELECT * FROM transcript_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to read transcript_settings for verification")?;

        let row = match row {
            Some(r) => r,
            None => return Ok(()),
        };

        let checks: Vec<(&str, &str)> = vec![
            ("whisper", "whisperApiKey"),
            ("deepgram", "deepgramApiKey"),
            ("elevenLabs", "elevenLabsApiKey"),
            ("groq", "groqApiKey"),
            ("openai", "openaiApiKey"),
        ];

        for (provider, column) in &checks {
            let expected: Option<String> = read_column(&row, column);
            let key = refs::transcript_provider_key(provider);

            let expected_val = match expected {
                Some(ref v) if !v.trim().is_empty() => v.clone(),
                _ => continue,
            };

            let stored = store
                .get(&key)
                .await
                .map_err(|e| anyhow!("verification read failed for {}: {}", key.as_str(), e))?;

            match stored {
                Some(ref s) if s == &expected_val => {}
                Some(s) => {
                    return Err(anyhow!(
                        "verification mismatch for {}: expected {:?}, stored {:?}",
                        key.as_str(),
                        expected_val,
                        s
                    ));
                }
                None => {
                    return Err(anyhow!(
                        "verification: transcript secret {} expected but not found",
                        key.as_str()
                    ));
                }
            }
        }

        Ok(())
    }

    async fn verify_kg_secrets(pool: &SqlitePool, store: &(dyn SecretStore + Sync)) -> Result<()> {
        let row = match Self::fetch_settings_row(pool).await? {
            Some(r) => r,
            None => return Ok(()),
        };

        let json: Option<String> = row.try_get("knowledge_graph_settings").ok().flatten();

        let json = match json {
            Some(j) => j,
            None => return Ok(()),
        };

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();

        let expected_key = parsed
            .get("profiles")
            .and_then(|p| p.as_array())
            .and_then(|profiles| {
                profiles.iter().find_map(|p| {
                    p.get("api_key")
                        .or_else(|| p.get("apiKey"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                })
            });

        let expected_key = match expected_key {
            Some(v) if !v.trim().is_empty() => Some(v.to_string()),
            _ => return Ok(()),
        };

        let key = refs::knowledge_graph_key();
        let stored = store
            .get(&key)
            .await
            .map_err(|e| anyhow!("verification read failed for knowledge-graph: {}", e))?;

        match (&expected_key, &stored) {
            (Some(el), Some(s)) if el == s => {}
            (Some(_el), None) => {
                return Err(anyhow!(
                    "verification: knowledge-graph secret expected but not found"
                ));
            }
            (Some(el), Some(s)) => {
                return Err(anyhow!(
                    "verification mismatch for knowledge-graph: expected {:?}, stored {:?}",
                    el,
                    s
                ));
            }
            _ => {}
        }

        Ok(())
    }

    /// Fetch the settings row, returning None on missing table.
    async fn fetch_settings_row(pool: &SqlitePool) -> Result<Option<sqlx::sqlite::SqliteRow>> {
        sqlx::query("SELECT * FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to fetch settings row")
    }

    // ── scrub ─────────────────────────────────────────────────────────────

    /// Scrub (set to NULL) all secret columns in the legacy SQLite tables.
    ///
    /// **Does not drop tables or delete rows** — only NULLs out columns that
    /// previously held API keys.
    async fn scrub(pool: &SqlitePool) -> Result<()> {
        Self::scrub_settings(pool).await?;
        Self::scrub_transcript_settings(pool).await?;
        Self::scrub_knowledge_graph(pool).await?;
        Ok(())
    }

    async fn scrub_settings(pool: &SqlitePool) -> Result<()> {
        // Only scrub if the table and row exist.
        let exists = sqlx::query_scalar::<_, String>("SELECT id FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to check settings table before scrub")?;

        if exists.is_none() {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE settings SET
                groqApiKey = NULL,
                openaiApiKey = NULL,
                anthropicApiKey = NULL,
                ollamaApiKey = NULL,
                openRouterApiKey = NULL,
                customOpenAIConfig = NULL
            WHERE id = (SELECT id FROM settings LIMIT 1)
            "#,
        )
        .execute(pool)
        .await
        .with_context(|| "failed to scrub settings table")?;

        info!("scrubbed legacy settings secret columns");
        Ok(())
    }

    async fn scrub_transcript_settings(pool: &SqlitePool) -> Result<()> {
        let exists = sqlx::query_scalar::<_, String>("SELECT id FROM transcript_settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to check transcript_settings table before scrub")?;

        if exists.is_none() {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE transcript_settings SET
                whisperApiKey = NULL,
                deepgramApiKey = NULL,
                elevenLabsApiKey = NULL,
                groqApiKey = NULL,
                openaiApiKey = NULL
            WHERE id = (SELECT id FROM transcript_settings LIMIT 1)
            "#,
        )
        .execute(pool)
        .await
        .with_context(|| "failed to scrub transcript_settings table")?;

        info!("scrubbed legacy transcript_settings secret columns");
        Ok(())
    }

    async fn scrub_knowledge_graph(pool: &SqlitePool) -> Result<()> {
        let exists = sqlx::query_scalar::<_, String>("SELECT id FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .or_else(|e| {
                if is_missing_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
            .with_context(|| "failed to check settings table before KG scrub")?;

        if exists.is_none() {
            return Ok(());
        }

        sqlx::query(
            "UPDATE settings SET knowledge_graph_settings = NULL WHERE id = (SELECT id FROM settings LIMIT 1)",
        )
        .execute(pool)
        .await
        .with_context(|| "failed to scrub knowledge_graph_settings column")?;

        info!("scrubbed legacy knowledge_graph_settings column");
        Ok(())
    }
}

// ── helpers ────────────────────────────────────────────────────────────────

/// Return `true` if the error indicates the table does not exist.
fn is_missing_table(e: &sqlx::Error) -> bool {
    e.to_string().contains("no such table")
}

/// Extract a column value as `Option<String>` from a dynamic row.
fn read_column(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column).ok().flatten()
}

/// Extract a column value, falling back to the default when the column
/// is NULL or absent.
fn read_column_string(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<String> {
    read_column(row, column)
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

    #[tokio::test]
    async fn extraction_on_empty_db_returns_ok() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let (store, _store_dir) = temp_secret_store();
        let (config_repo, _config_dir) = temp_config_repo();

        let result = LegacyConfigExtractor::extract(&pool, &store, &config_repo).await;
        assert!(result.is_ok(), "extraction should succeed on empty DB");

        let loaded = config_repo.load().unwrap();
        // Check that extraction populated the config without providers
        // (providers come from default.yml template, not legacy extraction)
        assert_eq!(loaded.summary.provider_id, "local");
    }

    #[tokio::test]
    async fn extraction_on_missing_tables_returns_ok() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let (store, _store_dir) = temp_secret_store();
        let (config_repo, _config_dir) = temp_config_repo();

        let result = LegacyConfigExtractor::extract(&pool, &store, &config_repo).await;
        assert!(result.is_ok(), "extraction should succeed with no tables");

        let loaded = config_repo.load().unwrap();
        assert_eq!(loaded.summary.provider_id, "local");
    }
}
