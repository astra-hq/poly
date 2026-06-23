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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphInsertTextResponse {
    pub track_id: KnowledgeGraphTrackId,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGraphQueryRequest {
    pub query: String,
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
    fn given_query_request_when_deserialized_then_uses_default_top_k() {
        let request: KnowledgeGraphQueryRequest =
            serde_json::from_str(r#"{"query":"find action items"}"#).expect("deserialize request");

        assert_eq!(request.query, "find action items");
        assert_eq!(request.top_k, 5);
    }

    #[test]
    fn given_track_state_when_serialized_then_uses_snake_case() {
        let json =
            serde_json::to_string(&KnowledgeGraphJobState::Completed).expect("serialize job state");

        assert_eq!(json, r#""completed""#);
    }
}
