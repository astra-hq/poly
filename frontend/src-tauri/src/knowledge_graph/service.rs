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

pub struct KnowledgeGraphIngestionService {
    pool: SqlitePool,
}

impl KnowledgeGraphIngestionService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
            let file_source =
                format!("resourcefully/meetings/{}/chunks/{}.txt", meeting_id, seq);

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

            // ── Call provider ─────────────────────────────────────
            let request = KnowledgeGraphInsertTextRequest {
                text: chunk.text.clone(),
                source: Some(file_source.clone()),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sqlx::SqlitePool;
    use std::sync::Mutex;

    use crate::knowledge_graph::provider::*;
    use crate::knowledge_graph::types::{
        KnowledgeGraphHealth, KnowledgeGraphInsertTextResponse,
        KnowledgeGraphPipelineStatus, KnowledgeGraphQueryRequest,
        KnowledgeGraphQueryResponse, KnowledgeGraphTrackId, KnowledgeGraphTrackStatus,
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
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "query",
            })
        }

        async fn pipeline_status(
            &self,
        ) -> KnowledgeGraphResult<KnowledgeGraphPipelineStatus> {
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

        let result = service
            .ingest_meeting(&provider, "meeting-1", "none")
            .await;

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

        assert_eq!(source, "resourcefully/meetings/meeting-src/chunks/0.txt");
    }
}
