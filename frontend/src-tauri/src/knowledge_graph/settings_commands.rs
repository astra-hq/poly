use log::info;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Runtime};

use crate::knowledge_graph::config::{
    self, KnowledgeGraphSettings,
};
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::state::AppState;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Load knowledge-graph settings from the `settings` table.
///
/// Returns defaults (with the built-in default profile) when no settings
/// have been persisted yet, so commands work out of the box.
pub(crate) async fn load_kg_settings(
    pool: &SqlitePool,
) -> Result<KnowledgeGraphSettings, String> {
    let row =
        sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                format!("Failed to query knowledge graph settings: {}", e)
            })?;

    match row {
        Some(row) => {
            let json: Option<String> =
                row.try_get("knowledge_graph_settings").ok().flatten();
            match json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    format!(
                        "Failed to parse knowledge graph settings: {}",
                        e
                    )
                }),
                None => Ok(KnowledgeGraphSettings::default()),
            }
        }
        None => Ok(KnowledgeGraphSettings::default()),
    }
}

/// Persist knowledge-graph settings as JSON into the `settings` table.
///
/// Uses UPSERT (`ON CONFLICT(id) DO UPDATE`) so the column is updated
/// whether or not a settings row already exists.
pub(crate) async fn save_kg_settings(
    pool: &SqlitePool,
    settings: &KnowledgeGraphSettings,
) -> Result<(), String> {
    // Validate before persisting
    config::validate(settings).map_err(|e| e.to_string())?;

    let json =
        serde_json::to_string(settings).map_err(|e| {
            format!("Failed to serialize knowledge graph settings: {}", e)
        })?;

    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel, knowledge_graph_settings)
        VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', $1)
        ON CONFLICT(id) DO UPDATE SET
            knowledge_graph_settings = excluded.knowledge_graph_settings
        "#,
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| {
        format!("Failed to save knowledge graph settings: {}", e)
    })?;

    Ok(())
}

// ── Commands ────────────────────────────────────────────────────────────

/// Retrieve the current knowledge graph settings.
///
/// Returns defaults when nothing has been persisted yet.
#[tauri::command]
pub async fn api_get_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<KnowledgeGraphSettings, String> {
    info!("api_get_knowledge_graph_settings called");
    let pool = state.db_manager.pool();
    load_kg_settings(pool).await
}

/// Save (validate and persist) knowledge graph settings.
///
/// Rejects empty profiles, duplicate IDs, invalid URLs, etc.
#[tauri::command]
pub async fn api_save_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: KnowledgeGraphSettings,
) -> Result<KnowledgeGraphSettings, String> {
    info!(
        "api_save_knowledge_graph_settings: {} profiles",
        settings.profiles.len()
    );
    let pool = state.db_manager.pool();
    save_kg_settings(pool, &settings).await?;
    // Re-read so the caller gets the canonical stored copy.
    load_kg_settings(pool).await
}

/// Health-check a specific profile by calling its LightRAG endpoint.
///
/// Returns `{ healthy: bool, version?: string }`.
#[tauri::command]
pub async fn api_test_knowledge_graph_profile<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<serde_json::Value, String> {
    info!(
        "api_test_knowledge_graph_profile: profile={}",
        profile_id
    );

    // ── Validate profile_id ──────────────────────────────────────
    if profile_id.is_empty() || profile_id.eq_ignore_ascii_case("none") {
        return Err(format!("Invalid profile_id: '{}'", profile_id));
    }

    let pool = state.db_manager.pool();
    let settings = load_kg_settings(pool).await?;

    let profile = settings
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| {
            format!(
                "Profile '{}' not found in knowledge graph settings",
                profile_id
            )
        })?;

    let provider = LightRagProvider::new(
        &profile.lightrag_url,
        profile.api_key.clone(),
    )
    .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    match provider.health().await {
        Ok(health) => {
            let mut result =
                serde_json::json!({ "healthy": health.healthy });
            if let Some(version) = &health.version {
                result["version"] = serde_json::Value::String(version.clone());
            }
            Ok(result)
        }
        Err(e) => Err(format!(
            "Health check failed for profile '{}': {}",
            profile_id, e
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::config::{
        KnowledgeGraphProfile, KnowledgeGraphSelection,
        ProfileKind,
    };
    use httpmock::MockServer;
    use serde_json::json;
    use sqlx::SqlitePool;

    /// Create an in-memory SQLite database with the app schema applied.
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    // ── load_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn load_returns_defaults_when_no_settings_row() {
        let pool = test_pool().await;

        // No row inserted — should return defaults.
        let settings = load_kg_settings(&pool).await.unwrap();
        assert_eq!(settings.profiles.len(), 1);
        assert_eq!(settings.profiles[0].id, "default");
        assert_eq!(settings.active_profile, KnowledgeGraphSelection::None);
    }

    #[tokio::test]
    async fn load_returns_defaults_when_column_is_null() {
        let pool = test_pool().await;

        // Insert row but leave knowledge_graph_settings NULL.
        sqlx::query(
            "INSERT INTO settings (id, provider, model, whisperModel) VALUES ('1', 'openai', 'gpt-4o', 'large-v3')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let settings = load_kg_settings(&pool).await.unwrap();
        assert_eq!(settings.profiles[0].id, "default");
        assert_eq!(settings.active_profile, KnowledgeGraphSelection::None);
    }

    #[tokio::test]
    async fn load_returns_persisted_settings() {
        let pool = test_pool().await;

        let original = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "prod-1".into(),
                name: "production".into(),
                kind: ProfileKind::Remote,
                lightrag_url: "http://kg.example.com".into(),
                api_key: Some("key-123".into()),
                notes: Some("main cluster".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("prod-1".into()),
        };
        save_kg_settings(&pool, &original).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded, original);
    }

    // ── save_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn save_rejects_invalid_settings() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("At least one profile is required"));
    }

    #[tokio::test]
    async fn save_rejects_empty_profile_name() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("Profile name cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_empty_lightrag_url() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "x".into(),
                lightrag_url: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("LightRAG URL cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_missing_active_profile() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "a".into(),
                name: "Alpha".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("b".into()),
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("Active profile 'b' not found"));
    }

    #[tokio::test]
    async fn save_accepts_valid_url_with_dummy_api_key() {
        let pool = test_pool().await;

        let valid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "local-1".into(),
                name: "local dev".into(),
                kind: ProfileKind::Local,
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("dummy-key".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("local-1".into()),
        };
        assert!(save_kg_settings(&pool, &valid).await.is_ok());
        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded, valid);
    }

    // ── Profile CRUD roundtrip ──────────────────────────────────────

    #[tokio::test]
    async fn profile_add_update_delete_roundtrip() {
        let pool = test_pool().await;

        // Start with defaults
        let initial = load_kg_settings(&pool).await.unwrap();
        assert_eq!(initial.profiles.len(), 1);

        // Add a second profile
        let mut settings = initial.clone();
        settings.profiles.push(KnowledgeGraphProfile {
            id: "remote-1".into(),
            name: "cloud kg".into(),
            kind: ProfileKind::Remote,
            lightrag_url: "https://kg.example.com".into(),
            api_key: Some("remote-key".into()),
            notes: Some("team server".into()),
            ..Default::default()
        });
        save_kg_settings(&pool, &settings).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.profiles[1].id, "remote-1");

        // Update the second profile (change name + URL)
        let mut updated = loaded.clone();
        if let Some(p) = updated.profiles.iter_mut().find(|p| p.id == "remote-1") {
            p.name = "cloud kg v2".into();
            p.lightrag_url = "https://kg2.example.com".into();
        }
        save_kg_settings(&pool, &updated).await.unwrap();

        let loaded2 = load_kg_settings(&pool).await.unwrap();
        let p = loaded2.profiles.iter().find(|p| p.id == "remote-1").unwrap();
        assert_eq!(p.name, "cloud kg v2");
        assert_eq!(p.lightrag_url, "https://kg2.example.com");

        // Delete the second profile
        let mut pruned = loaded2.clone();
        pruned.profiles.retain(|p| p.id != "remote-1");
        save_kg_settings(&pool, &pruned).await.unwrap();

        let loaded3 = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded3.profiles.len(), 1);
        assert_eq!(loaded3.profiles[0].id, "default");
    }

    // ── Health check tests with mock HTTP ───────────────────────────

    #[tokio::test]
    async fn health_test_healthy_with_mock_server() {
        let pool = test_pool().await;
        let server = MockServer::start();

        // Persist a profile pointing at the mock server
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "health-1".into(),
                name: "mock".into(),
                kind: ProfileKind::Local,
                lightrag_url: server.base_url(),
                api_key: Some("secret".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("health-1".into()),
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        // Mock the /health endpoint
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/health")
                .header("X-API-Key", "secret");
            then.status(200)
                .json_body(json!({"healthy": true, "version": "2.0.0"}));
        });

        // Resolve and call health through the provider
        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "health-1")
            .unwrap();
        let provider =
            LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
                .unwrap();
        let health = provider.health().await.unwrap();

        mock.assert();
        assert!(health.healthy);
        assert_eq!(health.version.as_deref(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn health_test_unhealthy_with_mock_server() {
        let pool = test_pool().await;
        let server = MockServer::start();

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "unhealthy-1".into(),
                name: "broken".into(),
                kind: ProfileKind::Remote,
                lightrag_url: server.base_url(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        // Mock endpoint returns 500
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/health");
            then.status(500).body("internal error");
        });

        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "unhealthy-1")
            .unwrap();
        let provider =
            LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
                .unwrap();
        let result = provider.health().await;

        mock.assert();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500") || err.contains("internal error"),
            "expected error to mention 500 or 'internal error', got: {err}");
    }

    #[tokio::test]
    async fn health_test_profile_not_found() {
        let pool = test_pool().await;

        // No profile with id "nonexistent"
        let loaded = load_kg_settings(&pool).await.unwrap();
        let found = loaded
            .profiles
            .iter()
            .find(|p| p.id == "nonexistent");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn health_test_connection_refused() {
        let pool = test_pool().await;

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "dead".into(),
                name: "unreachable".into(),
                kind: ProfileKind::Local,
                // Use a port that is almost certainly not bound
                lightrag_url: "http://127.0.0.1:19999".into(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "dead")
            .unwrap();
        let result = LightRagProvider::new(
            &profile.lightrag_url,
            profile.api_key.clone(),
        )
        .unwrap()
        .health()
        .await;

        assert!(result.is_err(), "expected connection error");
    }
}
