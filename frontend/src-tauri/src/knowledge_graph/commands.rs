use log::info;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Runtime};

use crate::knowledge_graph::config::KnowledgeGraphSettings;
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::service::{IngestionSummary, KnowledgeGraphIngestionService};
use crate::state::AppState;

/// Load knowledge-graph settings from the settings table.
///
/// Returns defaults (with the built-in default profile) when no settings
/// have been persisted yet, so the command works out of the box.
async fn load_kg_settings(pool: &SqlitePool) -> Result<KnowledgeGraphSettings, String> {
    let row = sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to query knowledge graph settings: {}", e))?;

    match row {
        Some(row) => {
            let json: Option<String> = row.try_get("knowledge_graph_settings").ok().flatten();
            match json {
                Some(json) => serde_json::from_str(&json)
                    .map_err(|e| format!("Failed to parse knowledge graph settings: {}", e)),
                None => Ok(KnowledgeGraphSettings::default()),
            }
        }
        None => Ok(KnowledgeGraphSettings::default()),
    }
}

#[tauri::command]
pub async fn api_ingest_meeting_to_knowledge_graph<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    profile_id: String,
) -> Result<IngestionSummary, String> {
    info!(
        "api_ingest_meeting_to_knowledge_graph: meeting={}, profile={}",
        meeting_id, profile_id
    );

    // ── Validate profile_id ──────────────────────────────────────
    if profile_id.is_empty() || profile_id.eq_ignore_ascii_case("none") {
        return Err(format!("Invalid profile_id: '{}'", profile_id));
    }

    let pool = state.db_manager.pool();

    // ── Load KG settings ─────────────────────────────────────────
    let settings = load_kg_settings(pool).await?;

    // ── Resolve profile ──────────────────────────────────────────
    let profile = settings
        .profiles
        .iter()
        .find(|p| p.name == profile_id)
        .ok_or_else(|| {
            format!(
                "Profile '{}' not found in knowledge graph settings (available: {:?})",
                profile_id,
                settings
                    .profiles
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
            )
        })?;

    // ── Create provider ──────────────────────────────────────────
    let provider = LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    // ── Run ingestion ────────────────────────────────────────────
    let service = KnowledgeGraphIngestionService::new(pool.clone());
    service
        .ingest_meeting(&provider, &meeting_id, &profile_id)
        .await
}
