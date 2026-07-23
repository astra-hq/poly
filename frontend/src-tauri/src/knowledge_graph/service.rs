use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::database::models::Transcript;
use crate::knowledge_graph::chunker::{TranscriptChunker, TranscriptRow};
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::knowledge_graph::types::KnowledgeGraphInsertTextRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedChunk {
    pub sequence: usize,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionSummary {
    pub submitted_count: usize,
    pub already_submitted_count: usize,
    pub failed_count: usize,
    pub failed_chunks: Vec<FailedChunk>,
    pub meeting_id: String,
    pub profile_id: String,
}

/// Snapshot of a meeting's knowledge-graph ingestion ledger.
///
/// Returned by `api_get_meeting_knowledge_graph_status`.  Always succeeds
/// (returns zeroes when no profile is selected or the ledger is empty) —
/// failures only happen for infrastructure errors (DB down, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingKnowledgeGraphStatus {
    pub meeting_id: String,
    /// The profile that would be used for indexing (meeting-specific
    /// selection, falling back to global active profile).  `None` when
    /// no profile is selected anywhere.
    pub selected_profile_id: Option<String>,
    /// Whether the user can start indexing — a profile is selected and
    /// the meeting has at least one transcript row.
    pub is_indexing_available: bool,
    /// Number of chunks with status = 'submitted' in the ledger.
    pub submitted_count: u64,
    /// Number of chunks with status = 'failed' in the ledger.
    pub failed_count: u64,
    /// Number of chunks with status = 'pending' in the ledger.
    pub pending_count: u64,
    /// Total number of ledger rows for this meeting + effective profile.
    pub total_chunks: u64,
    /// Last N error messages from failed chunks (most recent first).
    pub last_errors: Vec<String>,
    /// Summary document ingestion status for this meeting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_document: Option<crate::knowledge_graph::types::SummaryDocumentStatus>,
    /// LightRAG pipeline status (pending/indexing/failed documents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_status: Option<crate::knowledge_graph::types::KnowledgeGraphPipelineStatus>,
    /// LightRAG endpoint URL for this profile (for UI links).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightrag_url: Option<String>,
}

pub struct KnowledgeGraphIngestionService {
    pool: SqlitePool,
}

impl KnowledgeGraphIngestionService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn transcript_chunk_file_source(meeting_id: &str, sequence: usize) -> String {
        format!("{}_{}", meeting_id, sequence)
    }

    /// Ingest all transcript chunks for a meeting into the knowledge graph.
    ///
    /// The ledger at `knowledge_graph_ingestion_chunks` guarantees that chunks
    /// with status `submitted` or `completed` are never re-sent to the provider
    /// (duplicate idempotency).  Chunks with status `pending` or `failed` are
    /// retried.  Each chunk is fingerprinted with SHA-256 so that content
    /// changes produce a new ledger row.
    pub async fn ingest_meeting(
        &self,
        provider: &dyn KnowledgeGraphProvider,
        meeting_id: &str,
        profile_id: &str,
    ) -> Result<IngestionSummary, String> {
        // ── Validate profile ──────────────────────────────────────
        if profile_id.is_empty() || profile_id.eq_ignore_ascii_case("none") {
            return Err(format!("Invalid profile_id: '{}'", profile_id));
        }

        // ── Load transcripts ──────────────────────────────────────
        let transcripts = sqlx::query_as::<_, Transcript>(
            "SELECT * FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time ASC",
        )
        .bind(meeting_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to load transcripts: {}", e))?;

        if transcripts.is_empty() {
            return Ok(IngestionSummary {
                submitted_count: 0,
                already_submitted_count: 0,
                failed_count: 0,
                failed_chunks: vec![],
                meeting_id: meeting_id.to_string(),
                profile_id: profile_id.to_string(),
            });
        }

        // ── Map DB rows → chunker rows ────────────────────────────
        let rows: Vec<TranscriptRow> = transcripts
            .iter()
            .map(|t| TranscriptRow {
                id: t.id.clone(),
                audio_start_time: t.audio_start_time,
                audio_end_time: t.audio_end_time,
                text: t.transcript.clone(),
            })
            .collect();

        // ── Chunk ─────────────────────────────────────────────────
        let chunker = TranscriptChunker::new(20.0);
        let chunks = chunker.chunk(rows);

        // ── Process each chunk ────────────────────────────────────
        let mut summary = IngestionSummary {
            submitted_count: 0,
            already_submitted_count: 0,
            failed_count: 0,
            failed_chunks: vec![],
            meeting_id: meeting_id.to_string(),
            profile_id: profile_id.to_string(),
        };

        for (seq, chunk) in chunks.iter().enumerate() {
            let fingerprint = {
                let mut hasher = Sha256::new();
                hasher.update(chunk.text.as_bytes());
                format!("{:x}", hasher.finalize())
            };
            let file_source = Self::transcript_chunk_file_source(meeting_id, seq);

            // ── Check ledger for already-submitted/completed chunk ─
            let existing = sqlx::query(
                "SELECT status FROM knowledge_graph_ingestion_chunks \
                 WHERE meeting_id = ? AND profile_id = ? AND chunk_sequence = ? AND chunk_fingerprint = ?",
            )
            .bind(meeting_id)
            .bind(profile_id)
            .bind(seq as i64)
            .bind(&fingerprint)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Ledger query failed: {}", e))?;

            if let Some(row) = &existing {
                let status: String = row.get("status");
                if status == "submitted" || status == "completed" {
                    summary.already_submitted_count += 1;
                    continue;
                }
                // status is 'pending' or 'failed' → fall through to retry
            }

            // ── Idempotent ledger insert ──────────────────────────
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT OR IGNORE INTO knowledge_graph_ingestion_chunks \
                 (meeting_id, profile_id, chunk_sequence, chunk_fingerprint, file_source, status, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)",
            )
            .bind(meeting_id)
            .bind(profile_id)
            .bind(seq as i64)
            .bind(&fingerprint)
            .bind(&file_source)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create ledger entry: {}", e))?;

            sqlx::query(
                "UPDATE knowledge_graph_ingestion_chunks \
                 SET file_source = ?, updated_at = ? \
                 WHERE meeting_id = ? AND profile_id = ? AND chunk_sequence = ? AND chunk_fingerprint = ? \
                 AND status IN ('pending', 'failed')",
            )
            .bind(&file_source)
            .bind(Utc::now().to_rfc3339())
            .bind(meeting_id)
            .bind(profile_id)
            .bind(seq as i64)
            .bind(&fingerprint)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to update ledger file source: {}", e))?;

            // ── Call provider ─────────────────────────────────────
            let request = KnowledgeGraphInsertTextRequest {
                text: chunk.text.clone(),
                source: Some(file_source.clone()),
                chunking: None,
            };

            match provider.insert_text(request).await {
                Ok(response) => {
                    sqlx::query(
                        "UPDATE knowledge_graph_ingestion_chunks \
                         SET status = 'submitted', track_id = ?, updated_at = ?, last_error = NULL \
                         WHERE meeting_id = ? AND profile_id = ? AND chunk_sequence = ? AND chunk_fingerprint = ?",
                    )
                    .bind(&response.track_id.0)
                    .bind(Utc::now().to_rfc3339())
                    .bind(meeting_id)
                    .bind(profile_id)
                    .bind(seq as i64)
                    .bind(&fingerprint)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to update ledger: {}", e))?;

                    summary.submitted_count += 1;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    sqlx::query(
                        "UPDATE knowledge_graph_ingestion_chunks \
                         SET status = 'failed', last_error = ?, updated_at = ? \
                         WHERE meeting_id = ? AND profile_id = ? AND chunk_sequence = ? AND chunk_fingerprint = ?",
                    )
                    .bind(&error_msg)
                    .bind(Utc::now().to_rfc3339())
                    .bind(meeting_id)
                    .bind(profile_id)
                    .bind(seq as i64)
                    .bind(&fingerprint)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to update ledger error: {}", e))?;

                    summary.failed_count += 1;
                    summary.failed_chunks.push(FailedChunk {
                        sequence: seq,
                        error: error_msg,
                    });
                }
            }
        }

        Ok(summary)
    }

    /// Query the ingestion ledger for a snapshot of chunk statuses.
    ///
    /// Lightweight: no chunking or provider calls.  Only reads from the
    /// `knowledge_graph_ingestion_chunks` table and the transcripts count.
    pub async fn get_meeting_status(
        &self,
        meeting_id: &str,
        profile_id: Option<&str>,
        lightrag_url: Option<&str>,
    ) -> Result<MeetingKnowledgeGraphStatus, String> {
        let is_indexing_available = if let Some(pid) = profile_id {
            if pid.is_empty() || pid.eq_ignore_ascii_case("none") {
                false
            } else {
                let transcript_count: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM transcripts WHERE meeting_id = ?")
                        .bind(meeting_id)
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|e| format!("Failed to count transcripts: {}", e))?;
                transcript_count > 0
            }
        } else {
            false
        };

        if profile_id.is_none()
            || profile_id.unwrap().is_empty()
            || profile_id.unwrap().eq_ignore_ascii_case("none")
        {
            return Ok(MeetingKnowledgeGraphStatus {
                meeting_id: meeting_id.to_string(),
                selected_profile_id: profile_id.map(|s| s.to_string()),
                is_indexing_available: false,
                submitted_count: 0,
                failed_count: 0,
                pending_count: 0,
                total_chunks: 0,
                last_errors: vec![],
                summary_document: None,
                pipeline_status: None,
                lightrag_url: lightrag_url.map(|s| s.to_string()),
            });
        }

        let pid = profile_id.unwrap();

        // ── Count chunks by status ──────────────────────────────
        let submitted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_graph_ingestion_chunks \
             WHERE meeting_id = ? AND profile_id = ? AND status = 'submitted'",
        )
        .bind(meeting_id)
        .bind(pid)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to count submitted chunks: {}", e))?;

        let failed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_graph_ingestion_chunks \
             WHERE meeting_id = ? AND profile_id = ? AND status = 'failed'",
        )
        .bind(meeting_id)
        .bind(pid)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to count failed chunks: {}", e))?;

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_graph_ingestion_chunks \
             WHERE meeting_id = ? AND profile_id = ? AND status = 'pending'",
        )
        .bind(meeting_id)
        .bind(pid)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to count pending chunks: {}", e))?;

        // ── Last errors ─────────────────────────────────────────
        let errors: Vec<String> = sqlx::query_scalar(
            "SELECT last_error FROM knowledge_graph_ingestion_chunks \
             WHERE meeting_id = ? AND profile_id = ? AND last_error IS NOT NULL \
             ORDER BY updated_at DESC LIMIT 5",
        )
        .bind(meeting_id)
        .bind(pid)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch last errors: {}", e))?;

        let summary_doc = self.get_summary_document_status(meeting_id, pid).await?;

        let total = (submitted + failed + pending) as u64;

        Ok(MeetingKnowledgeGraphStatus {
            meeting_id: meeting_id.to_string(),
            selected_profile_id: Some(pid.to_string()),
            is_indexing_available,
            submitted_count: submitted as u64,
            failed_count: failed as u64,
            pending_count: pending as u64,
            total_chunks: total,
            last_errors: errors,
            summary_document: summary_doc,
            pipeline_status: None,
            lightrag_url: lightrag_url.map(|s| s.to_string()),
        })
    }

    pub async fn get_summary_document_status(
        &self,
        meeting_id: &str,
        profile_id: &str,
    ) -> Result<Option<crate::knowledge_graph::types::SummaryDocumentStatus>, String> {
        let row = sqlx::query(
            "SELECT status, file_source, error, updated_at, track_id, document_id \
             FROM knowledge_graph_summary_documents \
             WHERE meeting_id = ? AND profile_id = ?",
        )
        .bind(meeting_id)
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch summary document status: {}", e))?;

        match row {
            Some(r) => {
                let status: String = r.get("status");
                let state = match status.as_str() {
                    "ingested" => crate::knowledge_graph::types::SummaryDocumentState::Ingested,
                    "failed" => crate::knowledge_graph::types::SummaryDocumentState::Failed,
                    "deleted" => crate::knowledge_graph::types::SummaryDocumentState::Deleted,
                    _ => crate::knowledge_graph::types::SummaryDocumentState::Pending,
                };
                Ok(Some(crate::knowledge_graph::types::SummaryDocumentStatus {
                    state,
                    file_source: r.get("file_source"),
                    error: r.get("error"),
                    updated_at: r.get("updated_at"),
                    track_id: r.get("track_id"),
                    document_id: r.get("document_id"),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_summary_document_id(
        &self,
        meeting_id: &str,
        profile_id: &str,
    ) -> Result<Option<String>, String> {
        let row = sqlx::query_scalar::<_, Option<String>>(
            "SELECT document_id FROM knowledge_graph_summary_documents \
             WHERE meeting_id = ? AND profile_id = ?",
        )
        .bind(meeting_id)
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch summary document id: {}", e))?;

        Ok(row.flatten())
    }

    /// Track summary document ingestion result.
    pub async fn track_summary_document(
        &self,
        meeting_id: &str,
        profile_id: &str,
        file_source: &str,
        state: crate::knowledge_graph::types::SummaryDocumentState,
        error: Option<&str>,
        track_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let status = match state {
            crate::knowledge_graph::types::SummaryDocumentState::Pending => "pending",
            crate::knowledge_graph::types::SummaryDocumentState::Ingested => "ingested",
            crate::knowledge_graph::types::SummaryDocumentState::Failed => "failed",
            crate::knowledge_graph::types::SummaryDocumentState::Deleted => "deleted",
        };

        sqlx::query(
            "INSERT INTO knowledge_graph_summary_documents \
             (meeting_id, profile_id, file_source, status, error, track_id, document_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(meeting_id, profile_id) DO UPDATE SET \
             file_source = excluded.file_source, \
             status = excluded.status, \
             error = excluded.error, \
             track_id = excluded.track_id, \
             document_id = excluded.document_id, \
             updated_at = excluded.updated_at",
        )
        .bind(meeting_id)
        .bind(profile_id)
        .bind(file_source)
        .bind(status)
        .bind(error)
        .bind(track_id)
        .bind(document_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to track summary document: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sqlx::SqlitePool;
    use std::sync::Mutex;

    use crate::knowledge_graph::provider::*;
    use crate::knowledge_graph::types::{
        DocumentStatus, KnowledgeGraphHealth, KnowledgeGraphInsertTextResponse,
        KnowledgeGraphPipelineStatus, KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse,
        KnowledgeGraphTrackId, KnowledgeGraphTrackStatus,
    };

    /// A mock provider that records every `insert_text` call and can be
    /// configured to fail at a specific call index.
    struct MockProvider {
        calls: Mutex<Vec<(String, Option<String>)>>,
        fail_at: Option<usize>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail_at: None,
            }
        }

        fn fail_at(mut self, index: usize) -> Self {
            self.fail_at = Some(index);
            self
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }

        fn call_texts(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .map(|(t, _)| t.clone())
                .collect()
        }
    }

    #[async_trait]
    impl KnowledgeGraphProvider for MockProvider {
        async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth> {
            Ok(KnowledgeGraphHealth {
                healthy: true,
                version: Some("mock".into()),
            })
        }

        async fn delete_by_file_source(&self, _file_source: &str) -> KnowledgeGraphResult<()> {
            Ok(())
        }

        async fn delete_by_doc_ids(&self, _doc_ids: &[String]) -> KnowledgeGraphResult<()> {
            Ok(())
        }

        async fn insert_text(
            &self,
            request: KnowledgeGraphInsertTextRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse> {
            let call_index = {
                let mut calls = self.calls.lock().unwrap();
                let idx = calls.len();
                calls.push((request.text.clone(), request.source.clone()));
                idx
            };
            if self.fail_at == Some(call_index) {
                return Err(KnowledgeGraphProviderError::RequestFailed {
                    message: "mock failure".into(),
                });
            }
            Ok(KnowledgeGraphInsertTextResponse {
                track_id: KnowledgeGraphTrackId(format!("track-{}", call_index)),
                accepted: true,
            })
        }

        async fn query(
            &self,
            _request: KnowledgeGraphQueryRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphQueryResponse> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation { operation: "query" })
        }

        async fn pipeline_status(&self) -> KnowledgeGraphResult<KnowledgeGraphPipelineStatus> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "pipeline_status",
            })
        }

        async fn track_status(
            &self,
            _track_id: KnowledgeGraphTrackId,
        ) -> KnowledgeGraphResult<KnowledgeGraphTrackStatus> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "track_status",
            })
        }

        async fn list_documents(&self) -> KnowledgeGraphResult<Vec<DocumentStatus>> {
            Ok(vec![])
        }

        fn provider_name(&self) -> &'static str {
            "mock"
        }
    }

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    async fn ensure_meeting_exists(pool: &SqlitePool, meeting_id: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO meetings (id, title, created_at, updated_at) \
             VALUES (?, ?, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
        )
        .bind(meeting_id)
        .bind(format!("Meeting {}", meeting_id))
        .execute(pool)
        .await
        .expect("insert meeting");
    }

    async fn insert_transcript(
        pool: &SqlitePool,
        id: &str,
        meeting_id: &str,
        text: &str,
        audio_start_time: f64,
    ) {
        ensure_meeting_exists(pool, meeting_id).await;
        sqlx::query(
            "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(meeting_id)
        .bind(text)
        .bind("2025-01-01T00:00:00Z")
        .bind(audio_start_time)
        .execute(pool)
        .await
        .expect("insert transcript");
    }

    // ── Tests ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn rejects_none_profile_id_with_error() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);
        let provider = MockProvider::new();

        let result = service.ingest_meeting(&provider, "meeting-1", "none").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Invalid profile_id"),
            "expected profile rejection, got: {}",
            err
        );
        // Zero provider calls
        assert_eq!(provider.call_count(), 0);
    }

    #[tokio::test]
    async fn rejects_empty_profile_id() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);
        let provider = MockProvider::new();

        let result = service.ingest_meeting(&provider, "meeting-1", "").await;

        assert!(result.is_err());
        assert_eq!(provider.call_count(), 0);
    }

    #[tokio::test]
    async fn returns_zero_counts_when_no_transcripts() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);
        let provider = MockProvider::new();

        let summary = service
            .ingest_meeting(&provider, "meeting-empty", "default")
            .await
            .expect("summary");

        assert_eq!(summary.submitted_count, 0);
        assert_eq!(summary.already_submitted_count, 0);
        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.meeting_id, "meeting-empty");
    }

    #[tokio::test]
    async fn ingests_chunks_in_sequence() {
        let pool = setup_test_db().await;
        // Insert 3 transcripts spread across 40 seconds → 2 chunks with 20s interval
        insert_transcript(&pool, "t1", "meeting-1", "alpha", 0.0).await;
        insert_transcript(&pool, "t2", "meeting-1", "bravo", 5.0).await;
        insert_transcript(&pool, "t3", "meeting-1", "charlie", 25.0).await;

        let service = KnowledgeGraphIngestionService::new(pool);
        let provider = MockProvider::new();

        let summary = service
            .ingest_meeting(&provider, "meeting-1", "default")
            .await
            .expect("summary");

        assert_eq!(summary.submitted_count, 2);
        assert_eq!(summary.already_submitted_count, 0);
        assert_eq!(summary.failed_count, 0);

        // Provider was called in sequence with the correct chunk texts
        let texts = provider.call_texts();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], "alpha bravo");
        assert_eq!(texts[1], "charlie");

        // Verify ledger rows
        let rows = sqlx::query(
            "SELECT * FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ? ORDER BY chunk_sequence",
        )
        .bind("meeting-1")
        .fetch_all(&service.pool)
        .await
        .expect("ledger query");

        assert_eq!(rows.len(), 2);
        for row in &rows {
            let status: String = row.get("status");
            assert_eq!(status, "submitted");
        }
    }

    #[tokio::test]
    async fn duplicate_ingestion_is_idempotent() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-dup", "hello world", 0.0).await;

        let service = KnowledgeGraphIngestionService::new(pool.clone());

        // First ingestion
        let provider1 = MockProvider::new();
        let summary1 = service
            .ingest_meeting(&provider1, "meeting-dup", "default")
            .await
            .expect("first ingestion");

        assert_eq!(summary1.submitted_count, 1);
        assert_eq!(summary1.already_submitted_count, 0);

        // Second ingestion — same meeting, same profile, same chunks
        let provider2 = MockProvider::new();
        let summary2 = service
            .ingest_meeting(&provider2, "meeting-dup", "default")
            .await
            .expect("second ingestion");

        // All chunks should be detected as already submitted
        assert_eq!(summary2.submitted_count, 0);
        assert_eq!(summary2.already_submitted_count, 1);
        // Second provider should not be called
        assert_eq!(provider2.call_count(), 0);
    }

    #[tokio::test]
    async fn failed_chunk_is_retried_and_others_proceed() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-fail", "chunk-a", 0.0).await;
        insert_transcript(&pool, "t2", "meeting-fail", "chunk-b", 25.0).await;
        insert_transcript(&pool, "t3", "meeting-fail", "chunk-c", 45.0).await;

        // First attempt: chunk 1 (index 1) fails
        {
            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let provider = MockProvider::new().fail_at(1); // fail second chunk

            let summary = service
                .ingest_meeting(&provider, "meeting-fail", "default")
                .await
                .expect("first attempt");

            // Chunks 0 and 2 succeed, chunk 1 fails
            assert_eq!(summary.submitted_count, 2);
            assert_eq!(summary.failed_count, 1);
            assert_eq!(summary.failed_chunks.len(), 1);
            assert_eq!(summary.failed_chunks[0].sequence, 1);
        }

        // Verify ledger: 2 submitted, 1 failed
        let all_statuses = sqlx::query_scalar::<_, String>(
            "SELECT status FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ? ORDER BY chunk_sequence",
        )
        .bind("meeting-fail")
        .fetch_all(&pool)
        .await
        .expect("status query");
        assert_eq!(all_statuses, vec!["submitted", "failed", "submitted"]);

        // Second attempt: chunk 1 is retried (was 'failed'), succeeds now
        {
            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let provider = MockProvider::new();

            let summary = service
                .ingest_meeting(&provider, "meeting-fail", "default")
                .await
                .expect("second attempt");

            // Only chunk 1 is retried; chunks 0, 2 already submitted
            assert_eq!(summary.submitted_count, 1);
            assert_eq!(summary.already_submitted_count, 2);
            assert_eq!(summary.failed_count, 0);
        }

        // All statuses should now be 'submitted'
        let final_statuses = sqlx::query_scalar::<_, String>(
            "SELECT status FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ? ORDER BY chunk_sequence",
        )
        .bind("meeting-fail")
        .fetch_all(&pool)
        .await
        .expect("final status query");
        assert_eq!(final_statuses, vec!["submitted", "submitted", "submitted"]);
    }

    #[tokio::test]
    async fn different_profiles_independent_ledger() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-multi", "hello", 0.0).await;

        let service = KnowledgeGraphIngestionService::new(pool.clone());

        let provider_a = MockProvider::new();
        let summary_a = service
            .ingest_meeting(&provider_a, "meeting-multi", "profile-a")
            .await
            .expect("profile-a");
        assert_eq!(summary_a.submitted_count, 1);

        let provider_b = MockProvider::new();
        let summary_b = service
            .ingest_meeting(&provider_b, "meeting-multi", "profile-b")
            .await
            .expect("profile-b");
        assert_eq!(summary_b.submitted_count, 1);

        // Each profile has its own ledger entry
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ?",
        )
        .bind("meeting-multi")
        .fetch_one(&pool)
        .await
        .expect("count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn file_source_follows_expected_pattern() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-src", "text", 0.0).await;

        let service = KnowledgeGraphIngestionService::new(pool.clone());
        let provider = MockProvider::new();

        service
            .ingest_meeting(&provider, "meeting-src", "default")
            .await
            .expect("ingestion");

        let source = sqlx::query_scalar::<_, String>(
            "SELECT file_source FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ? AND chunk_sequence = 0",
        )
        .bind("meeting-src")
        .fetch_one(&pool)
        .await
        .expect("source");

        assert_eq!(source, "meeting-src_0");
    }

    #[tokio::test]
    async fn failed_retry_updates_legacy_file_source() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-src", "text", 0.0).await;

        let service = KnowledgeGraphIngestionService::new(pool.clone());
        let failing_provider = MockProvider::new().fail_at(0);
        let failed = service
            .ingest_meeting(&failing_provider, "meeting-src", "default")
            .await
            .expect("failed ingestion summary");
        assert_eq!(failed.failed_count, 1);

        sqlx::query(
            "UPDATE knowledge_graph_ingestion_chunks \
             SET file_source = 'resourcefully/meetings/meeting-src/chunks/0.txt' \
             WHERE meeting_id = ? AND profile_id = ? AND chunk_sequence = 0",
        )
        .bind("meeting-src")
        .bind("default")
        .execute(&pool)
        .await
        .expect("legacy source update");

        let retry_provider = MockProvider::new();
        service
            .ingest_meeting(&retry_provider, "meeting-src", "default")
            .await
            .expect("retry ingestion");

        let source = sqlx::query_scalar::<_, String>(
            "SELECT file_source FROM knowledge_graph_ingestion_chunks WHERE meeting_id = ? AND chunk_sequence = 0",
        )
        .bind("meeting-src")
        .fetch_one(&pool)
        .await
        .expect("source");

        assert_eq!(source, "meeting-src_0");
    }

    // ── get_meeting_status tests ─────────────────────────────────

    #[tokio::test]
    async fn status_returns_zeroes_when_no_profile() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);

        let status = service
            .get_meeting_status("meeting-1", None, None)
            .await
            .expect("status");

        assert_eq!(status.meeting_id, "meeting-1");
        assert_eq!(status.selected_profile_id, None);
        assert!(!status.is_indexing_available);
        assert_eq!(status.submitted_count, 0);
        assert_eq!(status.failed_count, 0);
        assert_eq!(status.pending_count, 0);
        assert_eq!(status.total_chunks, 0);
        assert!(status.last_errors.is_empty());
    }

    #[tokio::test]
    async fn status_returns_zeroes_for_none_profile() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);

        let status = service
            .get_meeting_status("meeting-1", Some("none"), None)
            .await
            .expect("status");

        assert_eq!(status.selected_profile_id, Some("none".to_string()));
        assert!(!status.is_indexing_available);
    }

    #[tokio::test]
    async fn status_reflects_ledger_after_ingestion() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-stat", "hello world", 0.0).await;

        // First, check status is zero before ingestion.
        let service = KnowledgeGraphIngestionService::new(pool.clone());
        let status_before = service
            .get_meeting_status("meeting-stat", Some("default"), None)
            .await
            .expect("status before");
        assert_eq!(status_before.submitted_count, 0);
        assert_eq!(status_before.total_chunks, 0);

        // Ingest.
        let provider = MockProvider::new();
        service
            .ingest_meeting(&provider, "meeting-stat", "default")
            .await
            .expect("ingestion");

        // Check status after.
        let status_after = service
            .get_meeting_status("meeting-stat", Some("default"), None)
            .await
            .expect("status after");
        assert_eq!(status_after.submitted_count, 1);
        assert_eq!(status_after.failed_count, 0);
        assert_eq!(status_after.pending_count, 0);
        assert_eq!(status_after.total_chunks, 1);
        assert!(status_after.is_indexing_available);
    }

    #[tokio::test]
    async fn status_shows_failed_and_errors() {
        let pool = setup_test_db().await;
        insert_transcript(&pool, "t1", "meeting-err", "chunk-a", 0.0).await;
        insert_transcript(&pool, "t2", "meeting-err", "chunk-b", 25.0).await;

        let service = KnowledgeGraphIngestionService::new(pool.clone());

        // Fail the first chunk.
        let provider = MockProvider::new().fail_at(0);
        service
            .ingest_meeting(&provider, "meeting-err", "default")
            .await
            .expect("ingestion with failure");

        let status = service
            .get_meeting_status("meeting-err", Some("default"), None)
            .await
            .expect("status");

        assert_eq!(status.submitted_count, 1); // second chunk succeeded
        assert_eq!(status.failed_count, 1); // first chunk failed
        assert_eq!(status.total_chunks, 2);
        assert_eq!(status.last_errors.len(), 1);
        assert!(status.last_errors[0].contains("mock failure"));
    }

    #[tokio::test]
    async fn status_is_indexing_available_false_when_no_transcripts() {
        let pool = setup_test_db().await;
        let service = KnowledgeGraphIngestionService::new(pool);

        let status = service
            .get_meeting_status("empty-meeting", Some("default"), None)
            .await
            .expect("status");

        assert_eq!(status.selected_profile_id, Some("default".to_string()));
        assert!(!status.is_indexing_available);
        assert_eq!(status.total_chunks, 0);
    }
}
