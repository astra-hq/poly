use async_trait::async_trait;
use reqwest::{Client, Method, RequestBuilder, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

use crate::knowledge_graph::provider::{
    KnowledgeGraphProvider, KnowledgeGraphProviderError, KnowledgeGraphResult,
};
use crate::knowledge_graph::types::{
    KnowledgeGraphHealth, KnowledgeGraphInsertTextRequest, KnowledgeGraphInsertTextResponse,
    KnowledgeGraphJobState, KnowledgeGraphPipelineStatus, KnowledgeGraphQueryRequest,
    KnowledgeGraphQueryResponse, KnowledgeGraphTrackId, KnowledgeGraphTrackStatus,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct LightRagProvider {
    base_url: Url,
    api_key: Option<String>,
    client: Client,
}

impl LightRagProvider {
    pub fn new(base_url: impl AsRef<str>, api_key: Option<String>) -> KnowledgeGraphResult<Self> {
        let base_url = Url::parse(base_url.as_ref().trim()).map_err(|error| {
            KnowledgeGraphProviderError::ProtocolError {
                message: format!("invalid LightRAG URL: {error}"),
            }
        })?;

        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| KnowledgeGraphProviderError::RequestFailed {
                message: format!("failed to build HTTP client: {error}"),
            })?;

        Ok(Self {
            base_url,
            api_key: api_key.and_then(|value| {
                let value = value.trim().to_string();
                (!value.is_empty()).then_some(value)
            }),
            client,
        })
    }

    fn build_url(&self, segments: &[&str]) -> KnowledgeGraphResult<Url> {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .map_err(|_| KnowledgeGraphProviderError::ProtocolError {
                message: "base LightRAG URL cannot be used as a path base".into(),
            })?
            .pop_if_empty()
            .extend(segments.iter().copied());
        Ok(url)
    }

    fn auth(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.api_key {
            Some(key) => request.header("X-API-Key", key.as_str()),
            None => request,
        }
    }

    async fn execute<T>(&self, request: RequestBuilder, method: Method, url: Url) -> KnowledgeGraphResult<T>
    where
        T: DeserializeOwned,
    {
        let endpoint = format!("{} {}", method, url.path());
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                KnowledgeGraphProviderError::RequestFailed {
                    message: format!("{endpoint} timed out after 30s"),
                }
            } else {
                KnowledgeGraphProviderError::RequestFailed {
                    message: format!("{endpoint} failed: {error}"),
                }
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(KnowledgeGraphProviderError::ProtocolError {
                message: if body.is_empty() {
                    format!("{endpoint} returned {status}")
                } else {
                    format!("{endpoint} returned {status}: {body}")
                },
            });
        }

        response.json().await.map_err(|error| KnowledgeGraphProviderError::ProtocolError {
            message: format!("{endpoint} returned invalid JSON: {error}"),
        })
    }

    async fn get<T>(&self, segments: &[&str]) -> KnowledgeGraphResult<T>
    where
        T: DeserializeOwned,
    {
        let url = self.build_url(segments)?;
        let request = self.auth(self.client.get(url.clone()));
        self.execute(request, Method::GET, url).await
    }

    async fn delete<B, T>(&self, segments: &[&str], body: &B) -> KnowledgeGraphResult<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = self.build_url(segments)?;
        let request = self.auth(self.client.delete(url.clone()).json(body));
        self.execute(request, Method::DELETE, url).await
    }

    async fn post<B, T>(&self, segments: &[&str], body: &B) -> KnowledgeGraphResult<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = self.build_url(segments)?;
        let request = self.auth(self.client.post(url.clone()).json(body));
        self.execute(request, Method::POST, url).await
    }
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    #[serde(default)]
    healthy: Option<bool>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

impl From<HealthResponse> for KnowledgeGraphHealth {
    fn from(value: HealthResponse) -> Self {
        let healthy = value.healthy.unwrap_or(
            matches!(value.status.as_deref(), Some("ok" | "healthy" | "up"))
        );
        Self { healthy, version: value.version }
    }
}

#[derive(Debug, Deserialize)]
struct InsertTextResponse {
    #[serde(alias = "id")]
    track_id: String,
    #[serde(default)]
    accepted: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TrackStatusResponse {
    track_id: String,
    documents: Vec<crate::knowledge_graph::types::TrackStatusDocument>,
    total_count: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    status_summary: BTreeMap<String, usize>,
}

#[async_trait]
impl KnowledgeGraphProvider for LightRagProvider {
    async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth> {
        self.get::<HealthResponse>(&["health"]).await.map(Into::into)
    }

    async fn delete_by_file_source(
        &self,
        file_source: &str,
    ) -> KnowledgeGraphResult<()> {
        let query_request = crate::knowledge_graph::types::DocumentQueryRequest {
            status_filter: None,
            status_filters: None,
            page: 1,
            page_size: 200,
            sort_field: "file_path".to_string(),
            sort_direction: "asc".to_string(),
        };

        let query_result: Result<crate::knowledge_graph::types::DocumentQueryResponse, _> =
            self.post(&["documents"], &query_request).await;

        let doc_id = match query_result {
            Ok(response) => response
                .documents
                .into_iter()
                .find(|doc| doc.file_path == file_source)
                .map(|doc| doc.id),
            Err(e) => {
                log::warn!("Document query failed ({}), falling back to direct delete", e);
                None
            }
        };

        let doc_id = match doc_id {
            Some(id) => id,
            None => {
                log::info!("Document with file_path '{}' not found in KG, trying direct delete", file_source);
                file_source.to_string()
            }
        };

        self.delete_by_doc_ids(&[doc_id]).await
    }

    async fn delete_by_doc_ids(
        &self,
        doc_ids: &[String],
    ) -> KnowledgeGraphResult<()> {
        use serde::Serialize;

        #[derive(Serialize)]
        struct DeleteRequest {
            doc_ids: Vec<String>,
            #[serde(rename = "delete_file")]
            delete_file: bool,
            #[serde(rename = "delete_llm_cache")]
            delete_llm_cache: bool,
        }

        #[derive(Deserialize)]
        struct DeleteResponse {}

        self.delete::<DeleteRequest, DeleteResponse>(
            &["documents", "delete_document"],
            &DeleteRequest {
                doc_ids: doc_ids.to_vec(),
                delete_file: false,
                delete_llm_cache: false,
            },
        )
        .await?;
        Ok(())
    }

    async fn insert_text(
        &self,
        request: KnowledgeGraphInsertTextRequest,
    ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse> {
        self.post::<_, InsertTextResponse>(&["documents", "text"], &request)
            .await
            .map(|response| KnowledgeGraphInsertTextResponse {
                track_id: KnowledgeGraphTrackId(response.track_id),
                accepted: response.accepted.unwrap_or(true),
            })
    }

    async fn query(
        &self,
        request: KnowledgeGraphQueryRequest,
    ) -> KnowledgeGraphResult<KnowledgeGraphQueryResponse> {
        self.post::<_, KnowledgeGraphQueryResponse>(&["query"], &request).await
    }

    async fn pipeline_status(&self) -> KnowledgeGraphResult<KnowledgeGraphPipelineStatus> {
        self.get::<KnowledgeGraphPipelineStatus>(&["documents", "pipeline_status"]).await
    }

    async fn track_status(
        &self,
        track_id: KnowledgeGraphTrackId,
    ) -> KnowledgeGraphResult<KnowledgeGraphTrackStatus> {
        self.get::<TrackStatusResponse>(&["documents", "track_status", &track_id.0])
            .await
            .map(|response| KnowledgeGraphTrackStatus {
                track_id: response.track_id,
                documents: response.documents,
                total_count: response.total_count,
                status_summary: response.status_summary,
            })
    }

    fn provider_name(&self) -> &'static str {
        "lightrag"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::types::QueryMode;
    use httpmock::Method::{DELETE, GET, POST};
    use httpmock::MockServer;
    use serde_json::json;

    #[tokio::test]
    async fn health_sends_auth_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/health").header("X-API-Key", "secret");
            then.status(200).json_body(json!({"healthy": true, "version": "1.0.0"}));
        });
        let provider = LightRagProvider::new(server.base_url(), Some("secret".into())).unwrap();

        let health = provider.health().await.unwrap();
        mock.assert();
        assert!(health.healthy);
        assert_eq!(health.version.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn insert_text_posts_document_and_maps_track_id() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/documents/text").header("X-API-Key", "secret");
            then.status(200).json_body(json!({"track_id": "track-123", "accepted": true}));
        });
        let provider = LightRagProvider::new(server.base_url(), Some("secret".into())).unwrap();

        let response = provider
            .insert_text(KnowledgeGraphInsertTextRequest { text: "hello".into(), source: None })
            .await
            .unwrap();
        mock.assert();
        assert_eq!(response.track_id.0, "track-123");
        assert!(response.accepted);
    }

    #[tokio::test]
    async fn query_parses_response_and_pipeline_status_uses_get() {
        let server = MockServer::start();
        let query_mock = server.mock(|when, then| {
            when.method(POST).path("/query");
            then.status(200).json_body(json!({"nodes": [], "edges": [], "answer": "ok"}));
        });
        let status_mock = server.mock(|when, then| {
            when.method(GET).path("/documents/pipeline_status");
            then.status(200).json_body(json!({"pending_documents": 1, "indexing_documents": 2, "failed_documents": 3}));
        });
        let provider = LightRagProvider::new(server.base_url(), None).unwrap();

        let query = provider
            .query(KnowledgeGraphQueryRequest { query: "find".into(), mode: Default::default(), top_k: 3 })
            .await
            .unwrap();
        let status = provider.pipeline_status().await.unwrap();
        query_mock.assert();
        status_mock.assert();
        assert_eq!(query.answer.as_deref(), Some("ok"));
        assert_eq!(status.pending_documents, 1);
    }

    #[tokio::test]
    async fn query_posts_mode_hybrid_in_request_body() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/query")
                .json_body(json!({"query":"find items","mode":"hybrid","top_k":5}));
            then.status(200).json_body(json!({"nodes": [], "edges": [], "answer": "ok"}));
        });
        let provider = LightRagProvider::new(server.base_url(), None).unwrap();

        provider
            .query(KnowledgeGraphQueryRequest {
                query: "find items".into(),
                mode: QueryMode::Hybrid,
                top_k: 5,
            })
            .await
            .unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn query_posts_mode_local_in_request_body() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/query")
                .json_body(json!({"query":"local search","mode":"local","top_k":10}));
            then.status(200).json_body(json!({"nodes": [], "edges": [], "answer": "local"}));
        });
        let provider = LightRagProvider::new(server.base_url(), None).unwrap();

        provider
            .query(KnowledgeGraphQueryRequest {
                query: "local search".into(),
                mode: QueryMode::Local,
                top_k: 10,
            })
            .await
            .unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn delete_by_file_source_queries_then_deletes_by_doc_id() {
        let server = MockServer::start();
        let query_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/documents")
                .header("X-API-Key", "secret");
            then.status(200).json_body(json!({
                "documents": [
                    {
                        "id": "doc-123",
                        "content_summary": "Summary",
                        "content_length": 100,
                        "status": "processed",
                        "created_at": "2025-01-01T00:00:00",
                        "updated_at": "2025-01-01T00:00:00",
                        "file_path": "meeting-summary-123"
                    }
                ],
                "pagination": {
                    "page": 1,
                    "page_size": 200,
                    "total_count": 1,
                    "total_pages": 1,
                    "has_next": false,
                    "has_prev": false
                },
                "status_counts": {}
            }));
        });
        let delete_mock = server.mock(|when, then| {
            when.method(DELETE)
                .path("/documents/delete_document")
                .header("X-API-Key", "secret")
                .json_body(json!({
                    "doc_ids": ["doc-123"],
                    "delete_file": false,
                    "delete_llm_cache": false
                }));
            then.status(200).json_body(json!({}));
        });
        let provider = LightRagProvider::new(server.base_url(), Some("secret".into())).unwrap();

        provider.delete_by_file_source("meeting-summary-123").await.unwrap();
        query_mock.assert();
        delete_mock.assert();
    }

    #[tokio::test]
    async fn track_status_maps_http_errors_to_protocol_errors() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/documents/track_status/track-1");
            then.status(500).body("boom");
        });
        let provider = LightRagProvider::new(server.base_url(), None).unwrap();

        let error = provider.track_status(KnowledgeGraphTrackId("track-1".into())).await.unwrap_err();
        mock.assert();
        assert!(matches!(error, KnowledgeGraphProviderError::ProtocolError { .. }));
        assert!(error.to_string().contains("500"));
        assert!(error.to_string().contains("boom"));
    }
}
