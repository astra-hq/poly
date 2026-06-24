use async_trait::async_trait;
use thiserror::Error;

use crate::knowledge_graph::types::{
    KnowledgeGraphHealth, KnowledgeGraphInsertTextRequest, KnowledgeGraphInsertTextResponse,
    KnowledgeGraphPipelineStatus, KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse,
    KnowledgeGraphTrackId, KnowledgeGraphTrackStatus,
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum KnowledgeGraphProviderError {
    #[error("request failed: {message}")]
    RequestFailed { message: String },
    #[error("protocol error: {message}")]
    ProtocolError { message: String },
    #[error("unsupported operation: {operation}")]
    UnsupportedOperation { operation: &'static str },
}

pub type KnowledgeGraphResult<T> = std::result::Result<T, KnowledgeGraphProviderError>;

#[async_trait]
pub trait KnowledgeGraphProvider: Send + Sync {
    async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth>;

    async fn insert_text(
        &self,
        request: KnowledgeGraphInsertTextRequest,
    ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse>;

    async fn delete_by_file_source(
        &self,
        file_source: &str,
    ) -> KnowledgeGraphResult<()>;

    async fn query(
        &self,
        request: KnowledgeGraphQueryRequest,
    ) -> KnowledgeGraphResult<KnowledgeGraphQueryResponse>;

    async fn pipeline_status(&self) -> KnowledgeGraphResult<KnowledgeGraphPipelineStatus>;

    async fn track_status(
        &self,
        track_id: KnowledgeGraphTrackId,
    ) -> KnowledgeGraphResult<KnowledgeGraphTrackStatus>;

    fn provider_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::types::{
        KnowledgeGraphEdge, KnowledgeGraphJobState, KnowledgeGraphNode, KnowledgeGraphNodeId,
        KnowledgeGraphQueryRequest, KnowledgeGraphQueryResponse, KnowledgeGraphTrackId,
        KnowledgeGraphTrackStatus,
    };
    use std::collections::BTreeMap;

    struct FakeProvider;

    #[async_trait]
    impl KnowledgeGraphProvider for FakeProvider {
        async fn health(&self) -> KnowledgeGraphResult<KnowledgeGraphHealth> {
            Ok(KnowledgeGraphHealth {
                healthy: true,
                version: Some("1.0.0".into()),
            })
        }

        async fn delete_by_file_source(
            &self,
            _file_source: &str,
        ) -> KnowledgeGraphResult<()> {
            Ok(())
        }

        async fn insert_text(
            &self,
            request: KnowledgeGraphInsertTextRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphInsertTextResponse> {
            Ok(KnowledgeGraphInsertTextResponse {
                track_id: KnowledgeGraphTrackId(format!("track:{}", request.text.len())),
                accepted: true,
            })
        }

        async fn query(
            &self,
            request: KnowledgeGraphQueryRequest,
        ) -> KnowledgeGraphResult<KnowledgeGraphQueryResponse> {
            let mut properties = BTreeMap::new();
            properties.insert("matched".into(), request.query.clone());

            Ok(KnowledgeGraphQueryResponse {
                answer: Some(format!("matched {}", request.query)),
                nodes: vec![KnowledgeGraphNode {
                    id: KnowledgeGraphNodeId("node-1".into()),
                    label: "Result".into(),
                    properties,
                }],
                edges: vec![KnowledgeGraphEdge {
                    source: KnowledgeGraphNodeId("node-1".into()),
                    target: KnowledgeGraphNodeId("node-2".into()),
                    relation: "related_to".into(),
                    properties: BTreeMap::new(),
                }],
            })
        }

        async fn pipeline_status(&self) -> KnowledgeGraphResult<KnowledgeGraphPipelineStatus> {
            Ok(KnowledgeGraphPipelineStatus {
                pending_documents: 1,
                indexing_documents: 2,
                failed_documents: 0,
            })
        }

        async fn track_status(
            &self,
            track_id: KnowledgeGraphTrackId,
        ) -> KnowledgeGraphResult<KnowledgeGraphTrackStatus> {
            Ok(KnowledgeGraphTrackStatus {
                track_id,
                state: KnowledgeGraphJobState::Completed,
                detail: Some("done".into()),
            })
        }

        fn provider_name(&self) -> &'static str {
            "fake"
        }
    }

    #[tokio::test]
    async fn given_fake_provider_when_query_then_returns_canned_response() {
        let provider = FakeProvider;

        let response = provider
            .query(KnowledgeGraphQueryRequest {
                query: "meeting summary".into(),
                mode: Default::default(),
                top_k: 3,
            })
            .await
            .expect("query response");

        assert_eq!(provider.provider_name(), "fake");
        assert_eq!(response.answer.as_deref(), Some("matched meeting summary"));
        assert_eq!(response.nodes.len(), 1);
        assert_eq!(response.edges.len(), 1);
    }

    #[tokio::test]
    async fn given_fake_provider_when_track_status_then_returns_completed_state() {
        let provider = FakeProvider;

        let status = provider
            .track_status(KnowledgeGraphTrackId("track-123".into()))
            .await
            .expect("track status");

        assert_eq!(status.track_id.0, "track-123");
        assert_eq!(status.state, KnowledgeGraphJobState::Completed);
    }

    #[test]
    fn given_provider_error_when_displayed_then_contains_message() {
        let error = KnowledgeGraphProviderError::RequestFailed {
            message: "timeout".into(),
        };

        assert_eq!(error.to_string(), "request failed: timeout");
    }
}
