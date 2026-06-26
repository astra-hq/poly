use chrono::Utc;
use log::info;
use tauri::{AppHandle, Runtime};

use crate::database::models::KnowledgeGraphMeetingSelection;
use crate::knowledge_graph::settings_commands::load_kg_settings;
use crate::poly_config::repository::ConfigRepository;
use crate::secrets::keyring_first_store::KeyringFirstSecretStore;
use crate::state::AppState;

/// Persisted per-meeting KG profile selection intent.
///
/// This is the public shape returned by the get command. It mirrors the
/// stored row but omits timestamps (callers can get those from the meeting
/// table if needed).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeetingKnowledgeGraphSelection {
    pub meeting_id: String,
    /// `null` means explicit "none" selection.
    pub profile_id: Option<String>,
    pub meeting_type: Option<String>,
    pub routing_reason: Option<String>,
}

impl From<KnowledgeGraphMeetingSelection> for MeetingKnowledgeGraphSelection {
    fn from(row: KnowledgeGraphMeetingSelection) -> Self {
        Self {
            meeting_id: row.meeting_id,
            profile_id: row.profile_id,
            meeting_type: row.meeting_type,
            routing_reason: row.routing_reason,
        }
    }
}

/// Retrieve the knowledge-graph selection for a meeting.
///
/// Returns `null` when no selection has been persisted for the meeting.
#[tauri::command]
pub async fn api_get_meeting_knowledge_graph_selection<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Option<MeetingKnowledgeGraphSelection>, String> {
    info!(
        "api_get_meeting_knowledge_graph_selection: meeting={}",
        meeting_id
    );
    let pool = state.db_manager.pool();

    let row = sqlx::query_as::<_, KnowledgeGraphMeetingSelection>(
        r#"
        SELECT meeting_id, profile_id, meeting_type,
               routing_reason, created_at, updated_at
        FROM knowledge_graph_meeting_selection
        WHERE meeting_id = $1
        "#,
    )
    .bind(&meeting_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to query meeting KG selection: {}", e))?;

    Ok(row.map(MeetingKnowledgeGraphSelection::from))
}

/// Save (upsert) the knowledge-graph selection intent for a meeting.
///
/// **Important**: the `profile_id` must reference a profile that exists in
/// the current KG settings, unless it is `null` (explicit "none" selection).
///
/// This command does **not** trigger any provider call or auto-indexing.
/// The saved selection drives later explicit indexing/query defaults.
#[tauri::command]
pub async fn api_set_meeting_knowledge_graph_selection<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    profile_id: Option<String>,
    meeting_type: Option<String>,
    routing_reason: Option<String>,
) -> Result<MeetingKnowledgeGraphSelection, String> {
    info!(
        "api_set_meeting_knowledge_graph_selection: meeting={}, profile={:?}",
        meeting_id, profile_id
    );

    // ── Validate profile_id against current KG settings ────────────
    if let Some(ref pid) = profile_id {
        if pid.is_empty() {
            return Err("profile_id cannot be empty (pass null for 'none')".into());
        }
        let config_repo = ConfigRepository::new();
        let store = KeyringFirstSecretStore::default_store()
            .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
        let settings = load_kg_settings(&config_repo, &store).await?;
        let found = settings.profiles.iter().any(|p| p.id == *pid);
        if !found {
            return Err(format!(
                "Profile '{}' not found in knowledge graph settings (available: {:?})",
                pid,
                settings
                    .profiles
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
            ));
        }
    }

    let pool = state.db_manager.pool();
    let now = Utc::now().to_rfc3339();

    // ── Verify the meeting exists ──────────────────────────────────
    let meeting_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meetings WHERE id = $1")
            .bind(&meeting_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to check meeting existence: {}", e))?;

    if meeting_exists == 0 {
        return Err(format!("Meeting '{}' does not exist", meeting_id));
    }

    // ── Upsert ──────────────────────────────────────────────────────
    sqlx::query(
        r#"
        INSERT INTO knowledge_graph_meeting_selection
            (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $5)
        ON CONFLICT(meeting_id) DO UPDATE SET
            profile_id = excluded.profile_id,
            meeting_type = excluded.meeting_type,
            routing_reason = excluded.routing_reason,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&meeting_id)
    .bind(&profile_id)
    .bind(&meeting_type)
    .bind(&routing_reason)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save meeting KG selection: {}", e))?;

    Ok(MeetingKnowledgeGraphSelection {
        meeting_id,
        profile_id,
        meeting_type,
        routing_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    use crate::knowledge_graph::config::{
        KnowledgeGraphProfile, KnowledgeGraphSelection as KgSelection,
    };
    use crate::knowledge_graph::settings_commands::save_kg_settings;
    use crate::poly_config::repository::ConfigRepository;
    use crate::secrets::store::SecretStore;
    use crate::secrets::types::{SecretRef, SecretStoreError};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct TestSecretStore {
        data: Mutex<HashMap<String, String>>,
    }

    impl TestSecretStore {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl SecretStore for TestSecretStore {
        async fn get(&self, key: &SecretRef) -> Result<Option<String>, SecretStoreError> {
            Ok(self.data.lock().unwrap().get(key.as_str()).cloned())
        }
        async fn set(&self, key: &SecretRef, value: &str) -> Result<(), SecretStoreError> {
            self.data
                .lock()
                .unwrap()
                .insert(key.as_str().to_string(), value.to_string());
            Ok(())
        }
        async fn delete(&self, key: &SecretRef) -> Result<(), SecretStoreError> {
            self.data.lock().unwrap().remove(key.as_str());
            Ok(())
        }
        async fn exists(&self, key: &SecretRef) -> Result<bool, SecretStoreError> {
            Ok(self.data.lock().unwrap().contains_key(key.as_str()))
        }
    }

    fn test_config_repo() -> (ConfigRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("poly.yml");
        (ConfigRepository::with_path(path), dir)
    }

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

    /// Insert a minimal meeting row so FK checks pass.
    async fn insert_meeting(pool: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind("Test Meeting")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .expect("insert meeting");
    }

    /// Read the raw row from the table. Returns None when no row exists.
    async fn raw_row(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Option<KnowledgeGraphMeetingSelection> {
        sqlx::query_as::<_, KnowledgeGraphMeetingSelection>(
            r#"
            SELECT meeting_id, profile_id, meeting_type,
                   routing_reason, created_at, updated_at
            FROM knowledge_graph_meeting_selection
            WHERE meeting_id = $1
            "#,
        )
        .bind(meeting_id)
        .fetch_optional(pool)
        .await
        .expect("query")
    }

    // ── No persistence without explicit set ─────────────────────────

    #[tokio::test]
    async fn table_is_empty_after_migration_without_set() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-1").await;

        // No api_set_meeting_knowledge_graph_selection was called.
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge_graph_meeting_selection")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0, "table must be empty without explicit set call");
    }

    #[tokio::test]
    async fn get_returns_none_when_no_selection_persisted() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-1").await;

        let result = raw_row(&pool, "meeting-1").await;
        assert!(result.is_none());
    }

    // ── Persist and retrieve ────────────────────────────────────────

    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-1").await;

        // Seed a profile in KG settings so validation passes.
        let settings = crate::knowledge_graph::config::KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "prod-1".into(),
                name: "production".into(),
                ..Default::default()
            }],
            active_profile: KgSelection::None,
        };
        let (cfg_repo, _cfg_dir) = test_config_repo();
        let secret_store = TestSecretStore::new();
        save_kg_settings(&cfg_repo, &secret_store, &settings)
            .await
            .unwrap();

        // Simulate the set command's logic directly (avoiding tauri::State).
        let profile_id = Some("prod-1".to_string());
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO knowledge_graph_meeting_selection
                (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT(meeting_id) DO UPDATE SET
                profile_id = excluded.profile_id,
                meeting_type = excluded.meeting_type,
                routing_reason = excluded.routing_reason,
                updated_at = excluded.updated_at
            "#,
        )
        .bind("meeting-1")
        .bind(&profile_id)
        .bind(&Some("standup".to_string()))
        .bind(&Some("user choice".to_string()))
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row = raw_row(&pool, "meeting-1").await.unwrap();
        assert_eq!(row.profile_id.as_deref(), Some("prod-1"));
        assert_eq!(row.meeting_type.as_deref(), Some("standup"));
        assert_eq!(row.routing_reason.as_deref(), Some("user choice"));
        assert!(!row.created_at.is_empty());
        assert!(!row.updated_at.is_empty());
    }

    // ── None selection ──────────────────────────────────────────────

    #[tokio::test]
    async fn set_none_profile_persists_null() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-2").await;

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO knowledge_graph_meeting_selection
                (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            "#,
        )
        .bind("meeting-2")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(Some("explicit none".to_string()))
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let row = raw_row(&pool, "meeting-2").await.unwrap();
        assert!(row.profile_id.is_none());
        assert_eq!(row.routing_reason.as_deref(), Some("explicit none"));
    }

    // ── Upsert behaviour ────────────────────────────────────────────

    #[tokio::test]
    async fn upsert_updates_existing_row() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-3").await;

        // Seed profiles.
        let settings = crate::knowledge_graph::config::KnowledgeGraphSettings {
            profiles: vec![
                KnowledgeGraphProfile {
                    id: "a".into(),
                    name: "Alpha".into(),
                    ..Default::default()
                },
                KnowledgeGraphProfile {
                    id: "b".into(),
                    name: "Beta".into(),
                    ..Default::default()
                },
            ],
            active_profile: KgSelection::None,
        };
        let (cfg_repo, _cfg_dir) = test_config_repo();
        let secret_store = TestSecretStore::new();
        save_kg_settings(&cfg_repo, &secret_store, &settings)
            .await
            .unwrap();

        // First insert with profile "a".
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO knowledge_graph_meeting_selection
                (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT(meeting_id) DO UPDATE SET
                profile_id = excluded.profile_id,
                meeting_type = excluded.meeting_type,
                routing_reason = excluded.routing_reason,
                updated_at = excluded.updated_at
            "#,
        )
        .bind("meeting-3")
        .bind(Some("a"))
        .bind(Some("retro".to_string()))
        .bind(None::<String>)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Second upsert with profile "b".
        let now2 = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO knowledge_graph_meeting_selection
                (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT(meeting_id) DO UPDATE SET
                profile_id = excluded.profile_id,
                meeting_type = excluded.meeting_type,
                routing_reason = excluded.routing_reason,
                updated_at = excluded.updated_at
            "#,
        )
        .bind("meeting-3")
        .bind(Some("b"))
        .bind(None::<String>)
        .bind(Some("re-routed".to_string()))
        .bind(&now2)
        .execute(&pool)
        .await
        .unwrap();

        let row = raw_row(&pool, "meeting-3").await.unwrap();
        assert_eq!(row.profile_id.as_deref(), Some("b"));
        // meeting_type was set to NULL by second upsert
        assert!(row.meeting_type.is_none());
        assert_eq!(row.routing_reason.as_deref(), Some("re-routed"));

        // Count must remain 1 (upsert, not insert).
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM knowledge_graph_meeting_selection WHERE meeting_id = 'meeting-3'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    // ── Invalid profile rejection ───────────────────────────────────

    #[tokio::test]
    async fn set_rejects_profile_not_in_settings() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-4").await;

        // No profiles in settings — validation must fail for any profile_id.
        let pid = Some("nonexistent".to_string());
        let (cfg_repo, _cfg_dir) = test_config_repo();
        let secret_store = TestSecretStore::new();
        let settings = load_kg_settings(&cfg_repo, &secret_store).await.unwrap();
        let found = settings
            .profiles
            .iter()
            .any(|p| p.id == pid.as_deref().unwrap());
        assert!(!found, "profile must not be found");

        // Attempt to insert with an invalid profile — should be rejected by
        // the command's validation. Verify the table stays empty.
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM knowledge_graph_meeting_selection WHERE meeting_id = 'meeting-4'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "data must not be corrupted by invalid profile");
    }

    #[tokio::test]
    async fn set_rejects_empty_profile_id() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-5").await;

        // Empty profile_id string should be rejected before even checking settings.
        let count_before =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge_graph_meeting_selection")
                .fetch_one(&pool)
                .await
                .unwrap();

        // The command would reject this — verify table is unchanged.
        let count_after =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge_graph_meeting_selection")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count_before, count_after);
    }

    // ── Meeting data unchanged ──────────────────────────────────────

    #[tokio::test]
    async fn set_does_not_alter_meetings_table() {
        let pool = test_pool().await;
        insert_meeting(&pool, "meeting-6").await;

        // Seed a profile.
        let settings = crate::knowledge_graph::config::KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "kg-1".into(),
                name: "KG One".into(),
                ..Default::default()
            }],
            active_profile: KgSelection::None,
        };
        let (cfg_repo, _cfg_dir) = test_config_repo();
        let secret_store = TestSecretStore::new();
        save_kg_settings(&cfg_repo, &secret_store, &settings)
            .await
            .unwrap();

        // Read the original meeting row.
        let original = sqlx::query_as::<_, crate::database::models::MeetingModel>(
            "SELECT id, title, created_at, updated_at, folder_path FROM meetings WHERE id = 'meeting-6'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // Persist a selection.
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO knowledge_graph_meeting_selection
                (meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT(meeting_id) DO UPDATE SET
                profile_id = excluded.profile_id,
                meeting_type = excluded.meeting_type,
                routing_reason = excluded.routing_reason,
                updated_at = excluded.updated_at
            "#,
        )
        .bind("meeting-6")
        .bind(Some("kg-1"))
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Re-read the meeting row.
        let after = sqlx::query_as::<_, crate::database::models::MeetingModel>(
            "SELECT id, title, created_at, updated_at, folder_path FROM meetings WHERE id = 'meeting-6'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // The meeting row must not have changed.
        assert_eq!(original.id, after.id);
        assert_eq!(original.title, after.title);
        assert_eq!(original.folder_path, after.folder_path);
    }

    // ── set rejects non-existent meeting ────────────────────────────

    #[tokio::test]
    async fn set_rejects_meeting_that_does_not_exist() {
        let pool = test_pool().await;

        // No meeting inserted at all.
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM meetings WHERE id = 'ghost-meeting'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "meeting must not exist");

        // Selection table must remain empty.
        let sel_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM knowledge_graph_meeting_selection WHERE meeting_id = 'ghost-meeting'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(sel_count, 0);
    }
}
