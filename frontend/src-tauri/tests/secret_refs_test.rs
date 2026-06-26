/// Integration tests for secret reference helpers.
use app_lib::secrets::refs;

#[test]
fn summary_provider_key_format_integration() {
    let key = refs::summary_provider_key("openai");
    assert_eq!(key.as_str(), "provider/summary/openai/api_key");
}

#[test]
fn summary_provider_key_lowercases_integration() {
    let key = refs::summary_provider_key("OpenAI");
    assert_eq!(key.as_str(), "provider/summary/openai/api_key");
}

#[test]
fn transcript_provider_key_format_integration() {
    let key = refs::transcript_provider_key("whisper");
    assert_eq!(key.as_str(), "provider/transcript/whisper/api_key");
}

#[test]
fn transcript_provider_key_lowercases_integration() {
    let key = refs::transcript_provider_key("Deepgram");
    assert_eq!(key.as_str(), "provider/transcript/deepgram/api_key");
}

#[test]
fn custom_openai_key_is_fixed_integration() {
    let key = refs::custom_openai_key();
    assert_eq!(key.as_str(), "provider/summary/custom-openai/api_key");
}

#[test]
fn knowledge_graph_key_is_fixed_integration() {
    let key = refs::knowledge_graph_key();
    assert_eq!(key.as_str(), "knowledge_graph/lightrag/api_key");
}

#[test]
fn all_refs_are_distinct() {
    // Every canonical ref must have a unique namespace.
    let mut seen = std::collections::HashSet::new();
    let providers = &["openai", "groq", "anthropic", "ollama"];

    for p in providers {
        let key = refs::summary_provider_key(p);
        assert!(
            seen.insert(key.as_str().to_string()),
            "duplicate namespace for summary provider '{}'",
            p
        );
    }

    let transcript_providers = &["whisper", "deepgram", "elevenLabs"];
    for p in transcript_providers {
        let key = refs::transcript_provider_key(p);
        assert!(
            seen.insert(key.as_str().to_string()),
            "duplicate namespace for transcript provider '{}'",
            p
        );
    }

    assert!(seen.insert(refs::custom_openai_key().as_str().to_string()));
    assert!(seen.insert(refs::knowledge_graph_key().as_str().to_string()));
}

#[test]
fn summary_transcript_namespaces_dont_overlap() {
    // A provider that exists in both summary and transcript should get
    // distinct namespaces.
    let s = refs::summary_provider_key("groq");
    let t = refs::transcript_provider_key("groq");
    assert_ne!(
        s.as_str(),
        t.as_str(),
        "summary and transcript refs for same provider must differ"
    );
}

#[test]
fn all_refs_use_valid_namespaces() {
    // If any of these panics, the namespace embedded in the canonical ref
    // contains an invalid character.
    let _ = refs::summary_provider_key("openai");
    let _ = refs::transcript_provider_key("whisper");
    let _ = refs::custom_openai_key();
    let _ = refs::knowledge_graph_key();
}
