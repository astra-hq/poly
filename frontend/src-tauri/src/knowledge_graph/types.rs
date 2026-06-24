use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnowledgeGraphDocumentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnowledgeGraphNodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnowledgeGraphTrackId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphNode {
    pub id: KnowledgeGraphNodeId,
    pub label: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphEdge {
    pub source: KnowledgeGraphNodeId,
    pub target: KnowledgeGraphNodeId,
    pub relation: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphInsertTextRequest {
    pub text: String,
    /// LightRAG requires `file_source` — maps to this field via serde rename.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "file_source")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphInsertTextResponse {
    pub track_id: KnowledgeGraphTrackId,
    pub accepted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryMode {
    Local,
    Global,
    Hybrid,
    Naive,
    Mix,
    Bypass,
}

impl Default for QueryMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

fn default_query_mode() -> QueryMode {
    QueryMode::Hybrid
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphQueryRequest {
    pub query: String,
    #[serde(default = "default_query_mode")]
    pub mode: QueryMode,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphQueryResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<KnowledgeGraphNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<KnowledgeGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphHealth {
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphPipelineStatus {
    pub pending_documents: usize,
    pub indexing_documents: usize,
    pub failed_documents: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeGraphJobState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphTrackStatus {
    pub track_id: KnowledgeGraphTrackId,
    pub state: KnowledgeGraphJobState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryDocumentState {
    Pending,
    Ingested,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryDocumentStatus {
    pub state: SummaryDocumentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn default_top_k() -> usize {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_node_without_properties_when_serialized_then_omits_properties() {
        let node = KnowledgeGraphNode {
            id: KnowledgeGraphNodeId("node-1".into()),
            label: "Speaker".into(),
            properties: BTreeMap::new(),
        };

        let json = serde_json::to_string(&node).expect("serialize node");

        assert!(json.contains("node-1"));
        assert!(!json.contains("properties"));
    }

    #[test]
    fn given_query_request_when_deserialized_then_uses_default_top_k_and_mode() {
        let request: KnowledgeGraphQueryRequest =
            serde_json::from_str(r#"{"query":"find action items"}"#).expect("deserialize request");

        assert_eq!(request.query, "find action items");
        assert_eq!(request.top_k, 5);
        assert_eq!(request.mode, QueryMode::Hybrid);
    }

    #[test]
    fn given_query_request_when_deserialized_with_mode_then_uses_explicit_mode() {
        let request: KnowledgeGraphQueryRequest =
            serde_json::from_str(r#"{"query":"find items","mode":"local","top_k":10}"#)
                .expect("deserialize request");

        assert_eq!(request.query, "find items");
        assert_eq!(request.mode, QueryMode::Local);
        assert_eq!(request.top_k, 10);
    }

    #[test]
    fn given_track_state_when_serialized_then_uses_snake_case() {
        let json =
            serde_json::to_string(&KnowledgeGraphJobState::Completed).expect("serialize job state");

        assert_eq!(json, r#""completed""#);
    }

    #[test]
    fn query_mode_local_serializes_as_local() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Local).unwrap(),
            r#""local""#
        );
    }

    #[test]
    fn query_mode_global_serializes_as_global() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Global).unwrap(),
            r#""global""#
        );
    }

    #[test]
    fn query_mode_hybrid_serializes_as_hybrid() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Hybrid).unwrap(),
            r#""hybrid""#
        );
    }

    #[test]
    fn query_mode_naive_serializes_as_naive() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Naive).unwrap(),
            r#""naive""#
        );
    }

    #[test]
    fn query_mode_mix_serializes_as_mix() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Mix).unwrap(),
            r#""mix""#
        );
    }

    #[test]
    fn query_mode_bypass_serializes_as_bypass() {
        assert_eq!(
            serde_json::to_string(&QueryMode::Bypass).unwrap(),
            r#""bypass""#
        );
    }

    #[test]
    fn query_mode_all_variants_roundtrip() {
        let variants = [
            (QueryMode::Local, "local"),
            (QueryMode::Global, "global"),
            (QueryMode::Hybrid, "hybrid"),
            (QueryMode::Naive, "naive"),
            (QueryMode::Mix, "mix"),
            (QueryMode::Bypass, "bypass"),
        ];

        for (variant, expected) in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, format!(r#""{}""#, expected));
            let roundtripped: QueryMode = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtripped, variant);
        }
    }
}
