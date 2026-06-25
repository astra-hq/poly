use super::types::SecretRef;

/// Canonical secret references for all known secret paths in the application.
///
/// These functions produce `SecretRef` values with guaranteed-valid
/// namespaces so callers can use them without handling construction errors.

/// `provider/summary/{provider}/api_key` — API key for a summary LLM provider.
pub fn summary_provider_key(provider: &str) -> SecretRef {
    let provider = provider.to_lowercase();
    SecretRef::new_unchecked(format!("provider/summary/{}/api_key", provider))
}

/// `provider/transcript/{provider}/api_key` — API key for a transcript provider.
pub fn transcript_provider_key(provider: &str) -> SecretRef {
    let provider = provider.to_lowercase();
    SecretRef::new_unchecked(format!("provider/transcript/{}/api_key", provider))
}

/// `provider/summary/custom-openai/api_key` — API key for a custom
/// OpenAI-compatible endpoint.
pub fn custom_openai_key() -> SecretRef {
    SecretRef::new_unchecked("provider/summary/custom-openai/api_key")
}

/// `knowledge_graph/lightrag/api_key` — API key for the LightRAG knowledge
/// graph instance (single-key legacy form).
pub fn knowledge_graph_key() -> SecretRef {
    SecretRef::new_unchecked("knowledge_graph/lightrag/api_key")
}

/// `knowledge_graph/lightrag/{profile_id}/api_key` — API key for a specific
/// knowledge graph profile.
pub fn knowledge_graph_profile_key(profile_id: &str) -> SecretRef {
    SecretRef::new_unchecked(format!(
        "knowledge_graph/lightrag/{}/api_key",
        profile_id.to_lowercase()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_provider_key_lowercases_input() {
        let r = summary_provider_key("OpenAI");
        assert_eq!(r.as_str(), "provider/summary/openai/api_key");
    }

    #[test]
    fn transcript_provider_key_format() {
        let r = transcript_provider_key("whisper");
        assert_eq!(r.as_str(), "provider/transcript/whisper/api_key");
    }

    #[test]
    fn custom_openai_key_is_canonical() {
        let r = custom_openai_key();
        assert_eq!(r.as_str(), "provider/summary/custom-openai/api_key");
    }

    #[test]
    fn knowledge_graph_key_is_canonical() {
        let r = knowledge_graph_key();
        assert_eq!(r.as_str(), "knowledge_graph/lightrag/api_key");
    }

    #[test]
    fn all_refs_are_valid() {
        // If any of these panics, the namespace is invalid.
        let _ = summary_provider_key("openai");
        let _ = transcript_provider_key("whisper");
        let _ = custom_openai_key();
        let _ = knowledge_graph_key();
    }

    #[test]
    fn distinct_refs_are_not_equal() {
        let a = summary_provider_key("openai");
        let b = transcript_provider_key("openai");
        assert_ne!(a, b);
    }
}
