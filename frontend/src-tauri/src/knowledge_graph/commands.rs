use log::info;
use tauri::{AppHandle, Runtime};

use crate::knowledge_graph::config::KnowledgeGraphSelection;
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::knowledge_graph::service::{
    IngestionSummary, KnowledgeGraphIngestionService, MeetingKnowledgeGraphStatus,
};
use crate::knowledge_graph::settings_commands::load_kg_settings;
use crate::resourcefully_config::repository::ConfigRepository;
use crate::secrets::keyring_first_store::KeyringFirstSecretStore;
use crate::knowledge_graph::types::{
    KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse, QueryMode,
};
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

    // ── Load KG settings ─────────────────────────────────────────
    let pool = state.db_manager.pool();
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

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
    _state: tauri::State<'_, AppState>,
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

    // ── Load KG settings ─────────────────────────────────────────
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

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
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

    // ── Resolve effective profile ───────────────────────────────
    // Priority: meeting-specific selection → global active profile.
    let effective_profile = {
        let selection_row = sqlx::query_as::<
            _,
            crate::database::models::KnowledgeGraphMeetingSelection,
        >(
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

    let lightrag_url = effective_profile.as_ref().and_then(|pid| {
        settings.profiles.iter().find(|p| p.id == *pid).map(|p| p.lightrag_url.clone())
    });

    let service = KnowledgeGraphIngestionService::new(pool.clone());
    service
        .get_meeting_status(&meeting_id, effective_profile.as_deref(), lightrag_url.as_deref())
        .await
}

#[tauri::command]
pub async fn api_get_knowledge_graph_pipeline_status<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<crate::knowledge_graph::types::KnowledgeGraphPipelineStatus, String> {
    info!("api_get_knowledge_graph_pipeline_status: profile={}", profile_id);

    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

    let profile = settings
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{}' not found", profile_id))?;

    let provider = LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    provider
        .pipeline_status()
        .await
        .map_err(|e| format!("Failed to fetch pipeline status: {}", e))
}

// ── Summary ingestion ────────────────────────────────────────────────────

const SUMMARY_FILE_SOURCE_PREFIX: &str = "resourcefully/meetings";

fn summary_file_source(meeting_id: &str) -> String {
    format!("meeting-summary-{}", meeting_id)
}

fn legacy_title_summary_file_source(meeting_id: &str, meeting_title: &str) -> String {
    let sanitized = meeting_title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    let name = if sanitized.is_empty() {
        meeting_id.to_string()
    } else {
        format!("{}_{}", sanitized, meeting_id)
    };
    format!("{}/{}/summary.md", SUMMARY_FILE_SOURCE_PREFIX, name)
}

fn summary_delete_file_sources(meeting_id: &str, meeting_title: &str) -> [String; 3] {
    [
        format!("{}/{}/summary.md", SUMMARY_FILE_SOURCE_PREFIX, meeting_id),
        legacy_title_summary_file_source(meeting_id, meeting_title),
        summary_file_source(meeting_id),
    ]
}

async fn fetch_meeting_title(pool: &sqlx::SqlitePool, meeting_id: &str) -> Result<String, String> {
    let meeting = sqlx::query_as::<_, crate::database::models::MeetingModel>(
        "SELECT id, title, created_at, updated_at, folder_path FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to fetch meeting: {}", e))?
    .ok_or_else(|| format!("Meeting '{}' not found", meeting_id))?;
    Ok(meeting.title)
}

fn resolve_effective_profile<'a>(
    selection_row: Option<&'a crate::database::models::KnowledgeGraphMeetingSelection>,
    settings: &'a crate::knowledge_graph::config::KnowledgeGraphSettings,
) -> Option<&'a str> {
    match selection_row {
        Some(row) => match row.profile_id.as_deref() {
            Some(pid) if !pid.is_empty() && !pid.eq_ignore_ascii_case("none") => Some(pid),
            _ => None,
        },
        None => match &settings.active_profile {
            KnowledgeGraphSelection::Profile(p) => Some(p.as_str()),
            KnowledgeGraphSelection::None => None,
        },
    }
}

fn extract_summary_markdown(result_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(result_json).ok()?;
    let markdown = value.get("markdown")?.as_str()?;
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SummaryIngestResult {
    pub meeting_id: String,
    pub profile_id: Option<String>,
    pub ingested: bool,
    pub error: Option<String>,
}

/// Auto-ingest the meeting summary into the configured Knowledge Graph.
///
/// Called automatically after summary generation completes.
/// Uses the meeting's KG profile selection (or the global active profile).
/// Inserts the summary markdown as a single document with file_source
/// `resourcefully/meetings/{meeting_id}/summary.md`.
#[tauri::command]
pub async fn api_ingest_summary_to_knowledge_graph<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<SummaryIngestResult, String> {
    info!(
        "api_ingest_summary_to_knowledge_graph: meeting={}",
        meeting_id
    );

    let pool = state.db_manager.pool();

    // ── Resolve effective profile ───────────────────────────────
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;
    let selection_row =
        sqlx::query_as::<_, crate::database::models::KnowledgeGraphMeetingSelection>(
            "SELECT meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at \
         FROM knowledge_graph_meeting_selection WHERE meeting_id = $1",
        )
        .bind(&meeting_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to query meeting KG selection: {}", e))?;

    let effective_profile = resolve_effective_profile(selection_row.as_ref(), &settings);

    let profile_id = match effective_profile {
        Some(pid) => pid.to_string(),
        None => {
            info!(
                "No KG profile selected for meeting {}, skipping summary ingest",
                meeting_id
            );
            return Ok(SummaryIngestResult {
                meeting_id,
                profile_id: None,
                ingested: false,
                error: None,
            });
        }
    };

    // ── Resolve profile config ──────────────────────────────────
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

    let provider = LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    // ── Get summary markdown from DB ────────────────────────────
    let process = sqlx::query_as::<_, crate::database::models::SummaryProcess>(
        "SELECT * FROM summary_processes WHERE meeting_id = $1",
    )
    .bind(&meeting_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to query summary: {}", e))?
    .ok_or_else(|| format!("No summary found for meeting '{}'", meeting_id))?;

    let result_json = process
        .result
        .ok_or_else(|| format!("Summary result is empty for meeting '{}'", meeting_id))?;

    let markdown = extract_summary_markdown(&result_json).ok_or_else(|| {
        format!(
            "Summary result has no markdown field for meeting '{}'",
            meeting_id
        )
    })?;

    // ── Insert to KG ────────────────────────────────────────────
    let file_source = summary_file_source(&meeting_id);
    let request = crate::knowledge_graph::types::KnowledgeGraphInsertTextRequest {
        text: markdown,
        source: Some(file_source.clone()),
    };

    match provider.insert_text(request).await {
        Ok(_) => {
            info!(
                "Summary ingested to knowledge graph for meeting {} (profile: {})",
                meeting_id, profile_id
            );

            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let _ = service
                .track_summary_document(
                    &meeting_id,
                    &profile_id,
                    &file_source,
                    crate::knowledge_graph::types::SummaryDocumentState::Ingested,
                    None,
                )
                .await;

            Ok(SummaryIngestResult {
                meeting_id,
                profile_id: Some(profile_id),
                ingested: true,
                error: None,
            })
        }
        Err(e) => {
            let error_msg = e.to_string();
            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let _ = service
                .track_summary_document(
                    &meeting_id,
                    &profile_id,
                    &file_source,
                    crate::knowledge_graph::types::SummaryDocumentState::Failed,
                    Some(&error_msg),
                )
                .await;

            Err(format!(
                "Failed to insert summary to knowledge graph: {}",
                error_msg
            ))
        }
    }
}

/// Delete the summary document from the Knowledge Graph (for regeneration).
///
/// Called before re-ingesting a new summary for the same meeting.
/// Uses file_source `resourcefully/meetings/{meeting_id}/summary.md`.
#[tauri::command]
pub async fn api_delete_summary_from_knowledge_graph<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<SummaryIngestResult, String> {
    info!(
        "api_delete_summary_from_knowledge_graph: meeting={}",
        meeting_id
    );

    let pool = state.db_manager.pool();

    // ── Resolve effective profile ───────────────────────────────
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;
    let selection_row =
        sqlx::query_as::<_, crate::database::models::KnowledgeGraphMeetingSelection>(
            "SELECT meeting_id, profile_id, meeting_type, routing_reason, created_at, updated_at \
         FROM knowledge_graph_meeting_selection WHERE meeting_id = $1",
        )
        .bind(&meeting_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to query meeting KG selection: {}", e))?;

    let effective_profile = resolve_effective_profile(selection_row.as_ref(), &settings);

    let profile_id = match effective_profile {
        Some(pid) => pid.to_string(),
        None => {
            info!(
                "No KG profile selected for meeting {}, skipping summary delete",
                meeting_id
            );
            return Ok(SummaryIngestResult {
                meeting_id,
                profile_id: None,
                ingested: false,
                error: None,
            });
        }
    };

    // ── Resolve profile config ──────────────────────────────────
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

    let provider = LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    // ── Get meeting title for document naming ──────────────────
    let meeting_title = fetch_meeting_title(pool, &meeting_id).await?;

    // ── Delete from KG (try both old and new file_source formats) ─
    // Old format: resourcefully/meetings/{meeting_id}/summary.md
    // New format: resourcefully/meetings/{title}_{meeting_id}/summary.md
    let file_sources = summary_delete_file_sources(&meeting_id, &meeting_title);
    let mut last_error = None;
    for fs in &file_sources {
        match provider.delete_by_file_source(fs).await {
            Ok(()) => {
                info!("Deleted summary from KG with file_source: {}", fs);
            }
            Err(e) => {
                info!("Summary deletion with file_source {} returned: {}", fs, e);
                last_error = Some(e.to_string());
            }
        }
    }

    let service = KnowledgeGraphIngestionService::new(pool.clone());
    let _ = service
        .track_summary_document(
            &meeting_id,
            &profile_id,
            &summary_file_source(&meeting_id),
            crate::knowledge_graph::types::SummaryDocumentState::Deleted,
            last_error.as_deref(),
        )
        .await;

    Ok(SummaryIngestResult {
        meeting_id,
        profile_id: Some(profile_id),
        ingested: true,
        error: last_error,
    })
}

#[cfg(test)]
mod summary_source_tests {
    use super::*;

    #[test]
    fn summary_file_source_uses_meeting_summary_id() {
        let meeting_id = "7e6fd8de-7c1b-4cb4-9878-8c4fdb7ab661";

        let source = summary_file_source(meeting_id);

        assert_eq!(
            source,
            "meeting-summary-7e6fd8de-7c1b-4cb4-9878-8c4fdb7ab661"
        );
    }

    #[test]
    fn summary_delete_file_sources_include_legacy_and_current_ids() {
        let meeting_id = "meeting-123";

        let sources = summary_delete_file_sources(meeting_id, "Planning / Review");

        assert_eq!(sources[0], "resourcefully/meetings/meeting-123/summary.md");
        assert_eq!(
            sources[1],
            "resourcefully/meetings/Planning _ Review_meeting-123/summary.md"
        );
        assert_eq!(sources[2], "meeting-summary-meeting-123");
    }
}
