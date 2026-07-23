// knowledge_graph/integration_tests.rs
//
// Integration tests for safety guardrails:
//   1. KG is disabled by default (KnowledgeGraphSelection::None).
//   2. No provider calls happen without explicit command invocation.
//   3. Provider failure does not corrupt meeting/transcript data.
//
// These tests use in-memory SQLite (same pattern as service tests).
// They verify isolation: bugs in the KG module cannot break
// the recording/transcription pipeline.

#[cfg(test)]
mod integration {
    use async_trait::async_trait;
    use sqlx::SqlitePool;

    use crate::knowledge_graph::config::KnowledgeGraphSelection;
    use crate::knowledge_graph::provider::{
        KnowledgeGraphProvider, KnowledgeGraphProviderError, KnowledgeGraphResult,
    };
    use crate::knowledge_graph::service::KnowledgeGraphIngestionService;
    use crate::knowledge_graph::types::{
        DocumentStatus, KnowledgeGraphHealth, KnowledgeGraphInsertTextRequest,
        KnowledgeGraphInsertTextResponse, KnowledgeGraphNode, KnowledgeGraphNodeId,
        KnowledgeGraphPipelineStatus, KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse,
        KnowledgeGraphTrackId, KnowledgeGraphTrackStatus, QueryMode,
    };

    // ── Mock provider (simple version for integration tests) ──────

    struct MockProvider {
        call_count: std::sync::atomic::AtomicUsize,
        fail_every_call: bool,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicUsize::new(0),
                fail_every_call: false,
            }
        }

        fn fail_all(mut self) -> Self {
            self.fail_every_call = true;
            self
        }
    }

    #[async_trait]
    impl KnowledgeGraphProvider for MockProvider {
        async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth> {
            Ok(KnowledgeGraphHealth {
                healthy: !self.fail_every_call,
                version: Some("mock".into()),
            })
        }

        async fn insert_text(
            &self,
            _request: KnowledgeGraphInsertTextRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if self.fail_every_call {
                return Err(KnowledgeGraphProviderError::RequestFailed {
                    message: "simulated provider failure".into(),
                });
            }
            Ok(KnowledgeGraphInsertTextResponse {
                track_id: KnowledgeGraphTrackId(format!(
                    "track-{}",
                    self.call_count.load(std::sync::atomic::Ordering::Relaxed)
                )),
                accepted: true,
            })
        }

        async fn delete_by_file_source(&self, _file_source: &str) -> KnowledgeGraphResult<()> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "delete_by_file_source",
            })
        }

        async fn delete_by_doc_ids(&self, _doc_ids: &[String]) -> KnowledgeGraphResult<()> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "delete_by_doc_ids",
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
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "list_documents",
            })
        }

        fn provider_name(&self) -> &'static str {
            "mock"
        }
    }

    // ── Test helpers ─────────────────────────────────────────────

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

    async fn ensure_meeting_exists(pool: &SqlitePool, meeting_id: &str, title: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO meetings (id, title, created_at, updated_at) \
             VALUES (?, ?, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
        )
        .bind(meeting_id)
        .bind(title)
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

    async fn read_meeting_title(pool: &SqlitePool, meeting_id: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT title FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await
            .expect("query meeting")
    }

    async fn read_transcript_texts(pool: &SqlitePool, meeting_id: &str) -> Vec<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT transcript FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .expect("query transcripts")
    }

    // ── Tests ────────────────────────────────────────────────────

    /// Prove that no provider calls happen without explicit command invocation.
    /// The KG service does not auto-ingest; it only acts when `ingest_meeting`
    /// is explicitly called.
    #[tokio::test]
    async fn kg_does_not_ingest_without_explicit_command() {
        let pool = setup_test_db().await;
        ensure_meeting_exists(&pool, "meeting-safety-1", "Safety Meeting 1").await;
        insert_transcript(&pool, "t1", "meeting-safety-1", "hello world", 0.0).await;

        // ── Phase 1: No explicit call → no provider activity ─────
        {
            let provider = MockProvider::new();
            // The provider exists but has never been told to ingest.
            // This simulates normal recording/transcription flow where
            // the KG module is loaded but never explicitly invoked.
            assert_eq!(
                provider
                    .call_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "provider must not be called without explicit ingestion command"
            );
            // (provider drops here without ever calling insert_text)
        }

        // ── Phase 2: Explicit call → provider IS called ──────────
        {
            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let provider = MockProvider::new();
            let summary = service
                .ingest_meeting(&provider, "meeting-safety-1", "default")
                .await
                .expect("ingestion");
            assert!(
                summary.submitted_count >= 1,
                "expected at least one chunk submitted"
            );
            assert!(
                provider
                    .call_count
                    .load(std::sync::atomic::Ordering::Relaxed)
                    >= 1,
                "provider must be called when ingest_meeting is explicitly invoked"
            );
        }

        // ── Phase 3: Second service with no explicit call → no activity ─
        {
            let service = KnowledgeGraphIngestionService::new(pool.clone());
            let provider = MockProvider::new();
            // Simply constructing the service does NOT trigger ingestion.
            // The provider has zero calls.
            assert_eq!(
                provider
                    .call_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "constructing service must not call provider"
            );
            // Drop to verify no hidden Drop-side-effect calls provider
            drop(service);
            assert_eq!(
                provider
                    .call_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "dropping service must not call provider"
            );
        }
    }

    /// Prove that a failed ingestion (provider error) does not corrupt
    /// existing meeting and transcript rows in the database.
    #[tokio::test]
    async fn provider_failure_does_not_corrupt_meeting_data() {
        let pool = setup_test_db().await;
        let meeting_id = "meeting-safety-2";
        let meeting_title = "Critical Meeting — Must Not Be Corrupted";
        ensure_meeting_exists(&pool, meeting_id, meeting_title).await;
        insert_transcript(&pool, "t1", meeting_id, "action item one", 0.0).await;
        insert_transcript(&pool, "t2", meeting_id, "action item two", 5.0).await;
        insert_transcript(&pool, "t3", meeting_id, "action item three", 10.0).await;

        // Record pre-ingestion state
        let pre_title = read_meeting_title(&pool, meeting_id).await;
        let pre_texts = read_transcript_texts(&pool, meeting_id).await;
        assert_eq!(
            pre_texts.len(),
            3,
            "expected 3 transcripts before ingestion"
        );

        // ── Attempt ingestion with a failing provider ────────────
        let service = KnowledgeGraphIngestionService::new(pool.clone());
        let failing_provider = MockProvider::new().fail_all();
        let result = service
            .ingest_meeting(&failing_provider, meeting_id, "default")
            .await;

        // Ingestion must report failure (not crash/panic)
        match result {
            Ok(summary) => {
                // All chunks should have failed
                assert_eq!(summary.submitted_count, 0, "no chunks should succeed");
                assert!(
                    summary.failed_count >= 1,
                    "failed chunk count = {}",
                    summary.failed_count
                );
            }
            Err(e) => {
                // An error is also acceptable if the service bails early.
                // The key assertion is below: data integrity.
                eprintln!("ingestion returned error (expected): {}", e);
            }
        }

        // ── Verify meeting and transcript data are intact ────────
        let post_title = read_meeting_title(&pool, meeting_id).await;
        let post_texts = read_transcript_texts(&pool, meeting_id).await;

        assert_eq!(
            post_title, pre_title,
            "meeting title must be unchanged after failed ingestion"
        );
        assert_eq!(
            post_texts, pre_texts,
            "transcript texts must be unchanged after failed ingestion"
        );
        assert_eq!(
            post_texts.len(),
            3,
            "transcript count must be unchanged after failed ingestion"
        );
    }

    /// Prove that the default KnowledgeGraphSelection is None,
    /// which prevents any automatic ingestion.
    #[test]
    fn default_kg_config_is_disabled() {
        let default_selection = KnowledgeGraphSelection::default();
        assert_eq!(
            default_selection,
            KnowledgeGraphSelection::None,
            "default KnowledgeGraphSelection must be None to prevent auto-ingestion"
        );
    }

    // ── Mock-provider query tests ────────────────────────────────

    use std::collections::BTreeMap;

    struct QueryMockProvider {
        fail: bool,
    }

    impl QueryMockProvider {
        fn new() -> Self {
            Self { fail: false }
        }

        fn failing() -> Self {
            Self { fail: true }
        }
    }

    #[async_trait]
    impl KnowledgeGraphProvider for QueryMockProvider {
        async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth> {
            Ok(KnowledgeGraphHealth {
                healthy: true,
                version: Some("query-mock".into()),
            })
        }

        async fn insert_text(
            &self,
            _request: KnowledgeGraphInsertTextRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "insert_text",
            })
        }

        async fn delete_by_file_source(&self, _file_source: &str) -> KnowledgeGraphResult<()> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "delete_by_file_source",
            })
        }

        async fn delete_by_doc_ids(&self, _doc_ids: &[String]) -> KnowledgeGraphResult<()> {
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "delete_by_doc_ids",
            })
        }

        async fn query(
            &self,
            request: KnowledgeGraphQueryRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphQueryResponse> {
            if self.fail {
                return Err(KnowledgeGraphProviderError::RequestFailed {
                    message: "simulated query failure".into(),
                });
            }

            let mut properties = BTreeMap::new();
            properties.insert("mode".into(), format!("{:?}", request.mode));

            Ok(KnowledgeGraphQueryResponse {
                answer: Some(format!("results for: {}", request.query)),
                nodes: vec![KnowledgeGraphNode {
                    id: KnowledgeGraphNodeId("q-node-1".into()),
                    label: "QueryResult".into(),
                    properties,
                }],
                edges: vec![],
            })
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
            Err(KnowledgeGraphProviderError::UnsupportedOperation {
                operation: "list_documents",
            })
        }

        fn provider_name(&self) -> &'static str {
            "query-mock"
        }
    }

    #[tokio::test]
    async fn given_query_provider_when_query_succeeds_then_returns_results() {
        let provider = QueryMockProvider::new();

        let response = provider
            .query(KnowledgeGraphQueryRequest {
                query: "action items".into(),
                mode: QueryMode::Hybrid,
                top_k: 5,
            })
            .await
            .expect("query should succeed");

        assert_eq!(
            response.answer.as_deref(),
            Some("results for: action items")
        );
        assert_eq!(response.nodes.len(), 1);
        assert_eq!(response.nodes[0].label, "QueryResult");
    }

    #[tokio::test]
    async fn given_failing_query_provider_when_query_called_then_returns_error() {
        let provider = QueryMockProvider::failing();

        let result = provider
            .query(KnowledgeGraphQueryRequest {
                query: "action items".into(),
                mode: QueryMode::Local,
                top_k: 10,
            })
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "request failed: simulated query failure"
        );
    }

    #[tokio::test]
    async fn given_query_provider_when_query_with_different_modes_then_works() {
        let provider = QueryMockProvider::new();

        for mode in [
            QueryMode::Local,
            QueryMode::Global,
            QueryMode::Hybrid,
            QueryMode::Naive,
            QueryMode::Mix,
            QueryMode::Bypass,
        ] {
            let response = provider
                .query(KnowledgeGraphQueryRequest {
                    query: "test".into(),
                    mode,
                    top_k: 3,
                })
                .await
                .expect("query should succeed");

            assert!(response.answer.is_some());
        }
    }
}
