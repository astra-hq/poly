use log::info;
use tauri::{AppHandle, Runtime};

use crate::knowledge_graph::config::KnowledgeGraphSelection;
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::knowledge_graph::service::{
    IngestionSummary, KnowledgeGraphIngestionService, MeetingKnowledgeGraphStatus,
};
use crate::knowledge_graph::settings_commands::load_kg_settings;
use crate::knowledge_graph::types::{KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse, QueryMode};
use crate::state::AppState;

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
        .find(|p| p.id == profile_id)
        .ok_or_else(|| {
            format!(
                "Profile '{}' not found in knowledge graph settings (available: {:?})",
                profile_id,
                settings
                    .profiles
                    .iter()
                    .map(|p| p.id.as_str())
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

#[tauri::command]
pub async fn api_query_knowledge_graph<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile_id: String,
    query: String,
    mode: Option<QueryMode>,
    top_k: Option<usize>,
) -> Result<KnowledgeGraphQueryResponse, String> {
    info!(
        "api_query_knowledge_graph: profile={}, query_len={}",
        profile_id,
        query.len()
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
        .find(|p| p.id == profile_id)
        .ok_or_else(|| {
            format!(
                "Profile '{}' not found in knowledge graph settings (available: {:?})",
                profile_id,
                settings
                    .profiles
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
            )
        })?;

    // ── Create provider ──────────────────────────────────────────
    let provider = LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    // ── Run query ────────────────────────────────────────────────
    let request = KnowledgeGraphQueryRequest {
        query,
        mode: mode.unwrap_or_default(),
        top_k: top_k.unwrap_or(5),
    };

    provider.query(request).await.map_err(|e| e.to_string())
}

/// Query the ingestion ledger for a meeting's knowledge-graph status.
///
/// Returns chunk counts by status, last errors, and a flag indicating
/// whether indexing is available.  Always succeeds (returns zeroes when
/// no profile is selected or the ledger is empty).
///
/// Profile resolution: meeting-specific selection first, then falls
/// back to the global active profile from KG settings.
#[tauri::command]
pub async fn api_get_meeting_knowledge_graph_status<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingKnowledgeGraphStatus, String> {
    info!(
        "api_get_meeting_knowledge_graph_status: meeting={}",
        meeting_id
    );

    let pool = state.db_manager.pool();
    let settings = load_kg_settings(pool).await?;

    // ── Resolve effective profile ───────────────────────────────
    // Priority: meeting-specific selection → global active profile.
    let effective_profile = {
        let selection_row = sqlx::query_as::<_, crate::database::models::KnowledgeGraphMeetingSelection>(
            "SELECT meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at \
             FROM knowledge_graph_meeting_selection WHERE meeting_id = $1",
        )
        .bind(&meeting_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to query meeting KG selection: {}", e))?;

        match selection_row {
            Some(row) => match row.profile_id {
                Some(pid) if !pid.is_empty() && !pid.eq_ignore_ascii_case("none") => Some(pid),
                _ => None,
            },
            None => match &settings.active_profile {
                KnowledgeGraphSelection::Profile(p) => Some(p.clone()),
                KnowledgeGraphSelection::None => None,
            },
        }
    };

    let service = KnowledgeGraphIngestionService::new(pool.clone());
    service
        .get_meeting_status(&meeting_id, effective_profile.as_deref())
        .await
}
