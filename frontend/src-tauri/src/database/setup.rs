use log::info;
use tauri::{AppHandle, Emitter, Manager};

use super::manager::DatabaseManager;
use crate::state::AppState;

/// Initialize database on app startup
/// Handles first launch detection and conditional initialization
pub async fn initialize_database_on_startup(app: &AppHandle) -> Result<(), String> {
    // Always create the database and manage AppState so commands work from startup
    let db_manager = DatabaseManager::new_from_app_handle(app)
        .await
        .map_err(|e| format!("Failed to initialize database manager: {}", e))?;

    // Ensure YAML config exists with defaults (creates on first run, loads existing otherwise)
    let config_repo = crate::resourcefully_config::ConfigRepository::new();
    config_repo
        .load_or_create_default()
        .map_err(|e| format!("Failed to initialize YAML config: {}", e))?;

    app.manage(AppState { db_manager, config_repo });
    log::info!("Database initialized successfully");

    // Check if this is the first launch (no database existed before)
    let is_first_launch = DatabaseManager::is_first_launch(app)
        .await
        .map_err(|e| format!("Failed to check first launch status: {}", e))?;

    if is_first_launch {
        log::info!("First launch detected - will notify window when ready");

        // Delay event emission to ensure window is ready and React listeners are registered
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            app_handle
                .emit("first-launch-detected", ())
                .expect("Failed to emit first-launch-detected event");
            log::info!("Emitted first-launch-detected after delay");
        });
    }

    // Run legacy extraction sequentially BEFORE any destructive cleanup,
    // and only drop old config tables after successful extraction.
    // This avoids racing with command reads and prevents silent data loss
    // when extraction fails.
    match run_legacy_extraction_if_needed(app).await {
        Ok(true) => {
            log::info!("Legacy extraction completed");
            if let Err(e) = drop_legacy_config_tables(app).await {
                log::warn!("Failed to drop legacy config tables: {}", e);
            }
        }
        Ok(false) => {
            log::info!("Legacy extraction skipped (no data or YAML already customized)");
        }
        Err(e) => {
            log::error!(
                "Legacy extraction failed, preserving legacy config tables: {}",
                e
            );
        }
    }

    Ok(())
}

/// Run legacy SQLite → YAML extraction if old config tables have data
/// and the YAML config is still in its default state (not customized).
///
/// Returns `Ok(true)` when extraction was actually performed, `Ok(false)` when
/// it was skipped (YAML already customized or no legacy data), or `Err` on failure.
async fn run_legacy_extraction_if_needed(app: &tauri::AppHandle) -> Result<bool, String> {
    use crate::resourcefully_config::config::ResourcefullyConfig;
    use crate::resourcefully_config::legacy_extraction::LegacyConfigExtractor;
    use crate::secrets::keyring_first_store::KeyringFirstSecretStore;

    let state = app.state::<AppState>();
    let pool = state.db_manager.pool();

    let config = state.config_repo.load()
        .map_err(|e| format!("Failed to load config for legacy check: {}", e))?;

    if config != ResourcefullyConfig::default() {
        info!("YAML config has custom values; skipping legacy extraction");
        return Ok(false);
    }

    let has_legacy_data = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM settings"
    )
    .fetch_one(pool)
    .await
    .map(|count| count > 0)
    .unwrap_or(false);

    if !has_legacy_data {
        info!("No legacy config data found; skipping extraction");
        return Ok(false);
    }

    info!("Legacy SQLite config tables detected; running extraction...");

    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Cannot create SecretStore for legacy extraction: {}", e))?;

    LegacyConfigExtractor::extract(pool, &store, &state.config_repo)
        .await
        .map_err(|e| format!("Legacy extraction failed: {}", e))?;

    info!("Legacy extraction completed successfully");
    Ok(true)
}

/// Drop the legacy `settings` and `transcript_settings` tables after
/// extraction has migrated their contents to YAML + SecretStore.
async fn drop_legacy_config_tables(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = state.db_manager.pool();
    drop_legacy_config_tables_with_pool(pool).await
}

/// Drop legacy config tables using a pool directly (testable without Tauri AppHandle).
pub async fn drop_legacy_config_tables_with_pool(pool: &sqlx::SqlitePool) -> Result<(), String> {
    sqlx::query("DROP TABLE IF EXISTS settings")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to drop settings table: {}", e))?;

    sqlx::query("DROP TABLE IF EXISTS transcript_settings")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to drop transcript_settings table: {}", e))?;

    info!("Dropped legacy config tables");
    Ok(())
}
