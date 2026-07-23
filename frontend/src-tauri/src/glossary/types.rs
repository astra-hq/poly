use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

pub const VALID_KINDS: &[&str] = &[
    "person",
    "team",
    "project",
    "code_name",
    "component",
    "acronym",
    "other",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryEntry {
    #[serde(default)]
    pub id: String,
    pub term: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pronunciation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Glossary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: Vec<GlossaryEntry>,
}

fn default_version() -> u32 {
    1
}

fn normalize_http_reference(reference: &str) -> Result<String, String> {
    let trimmed = reference.trim();
    let parsed = Url::parse(trimmed).map_err(|e| format!("invalid URL '{}': {}", trimmed, e))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed.to_string()),
        scheme => Err(format!(
            "reference '{}' must use http or https, got '{}'",
            trimmed, scheme
        )),
    }
}

fn is_valid_entry_id(id: &str) -> bool {
    id.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

impl Default for Glossary {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl Glossary {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut seen_terms: HashSet<String> = HashSet::new();

        for (i, entry) in self.entries.iter().enumerate() {
            let id = entry.id.trim();
            if id.is_empty() {
                return Err(format!("entry {}: id must be non-empty after trimming", i));
            }
            if !is_valid_entry_id(id) {
                return Err(format!(
                    "entry {}: id may contain only letters, numbers, hyphens, and underscores",
                    i
                ));
            }
            if !seen_ids.insert(id.to_string()) {
                return Err(format!("entry {}: duplicate id '{}'", i, entry.id));
            }

            let term = entry.term.trim();
            if term.is_empty() {
                return Err(format!(
                    "entry {}: term must be non-empty after trimming",
                    i
                ));
            }

            let term_lower = term.to_lowercase();
            if !seen_terms.insert(term_lower) {
                return Err(format!(
                    "entry {}: duplicate canonical term '{}' (case-insensitive)",
                    i, entry.term
                ));
            }

            if !VALID_KINDS.contains(&entry.kind.as_str()) {
                return Err(format!(
                    "entry {} ('{}'): kind '{}' is not one of: {}",
                    i,
                    term,
                    entry.kind,
                    VALID_KINDS.join(", ")
                ));
            }

            for reference in &entry.references {
                normalize_http_reference(reference).map_err(|e| {
                    format!("entry {} ('{}'): reference error: {}", i, term, e)
                })?;
            }

            let all_text = format!(
                "{} {} {} {:?} {:?} {:?} {}",
                entry.id,
                entry.term,
                entry.aliases.join(" "),
                entry.pronunciation,
                entry.definition,
                entry.notes,
                entry.references.join(" ")
            );
            let lower = all_text.to_lowercase();
            let secret_patterns = ["api_key", "apikey", "token", "password", "secret"];
            for pattern in &secret_patterns {
                if lower.contains(pattern) {
                    return Err(format!(
                        "entry {} ('{}'): may contain a secret (matched pattern '{}')",
                        i, term, pattern
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn normalize(&mut self) {
        for entry in &mut self.entries {
            entry.id = entry.id.trim().to_string();
            entry.term = entry.term.trim().to_string();
            entry.kind = entry.kind.trim().to_string();
            entry.pronunciation = entry
                .pronunciation
                .as_ref()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty());
            entry.definition = entry
                .definition
                .as_ref()
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty());
            entry.notes = entry
                .notes
                .as_ref()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty());
            entry.aliases = entry
                .aliases
                .iter()
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect();
            entry.references = entry
                .references
                .iter()
                .map(|reference| {
                    normalize_http_reference(reference)
                        .unwrap_or_else(|_| reference.trim().to_string())
                })
                .filter(|reference| !reference.is_empty())
                .collect();
        }
    }

    pub fn to_prompt_context(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        crate::glossary::render::render_prompt_context(self)
    }

    pub fn to_kg_document(&self) -> String {
        crate::glossary::render::render_kg_document(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, term: &str, kind: &str) -> GlossaryEntry {
        GlossaryEntry {
            id: id.to_string(),
            term: term.to_string(),
            kind: kind.to_string(),
            pronunciation: None,
            aliases: vec![],
            definition: None,
            notes: None,
            references: vec![],
        }
    }

    fn make_glossary(entries: Vec<GlossaryEntry>) -> Glossary {
        Glossary {
            version: 1,
            entries,
        }
    }

    #[test]
    fn default_glossary_is_empty() {
        let g = Glossary::default();
        assert_eq!(g.version, 1);
        assert!(g.entries.is_empty());
    }

    #[test]
    fn validate_accepts_valid_entries() {
        let g = make_glossary(vec![
            make_entry("person-sujith", "Sujith", "person"),
            make_entry("project-parakeet", "Parakeet", "project"),
            make_entry("acronym-tla", "TLA", "acronym"),
        ]);
        assert!(g.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_id() {
        let g = make_glossary(vec![make_entry("   ", "Sujith", "person")]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("id must be non-empty"));
    }

    #[test]
    fn validate_rejects_path_like_id() {
        let g = make_glossary(vec![make_entry("../entry", "Sujith", "person")]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("id may contain only"));
    }

    #[test]
    fn validate_rejects_duplicate_ids() {
        let g = make_glossary(vec![
            make_entry("entry-1", "Sujith", "person"),
            make_entry("entry-1", "Parakeet", "project"),
        ]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("duplicate id"));
    }

    #[test]
    fn validate_rejects_empty_term() {
        let g = make_glossary(vec![make_entry("entry-1", "   ", "person")]);
        let err = g.validate().unwrap_err();
        assert!(err.contains("must be non-empty"));
    }

    #[test]
    fn validate_rejects_duplicate_terms_case_insensitive() {
        let g = make_glossary(vec![
            make_entry("person-sujith", "Sujith", "person"),
            make_entry("person-sujith-2", "sujith", "person"),
        ]);
        let err = g.validate().unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn validate_rejects_invalid_kind() {
        let g = make_glossary(vec![make_entry("entry-1", "test", "invalid_kind")]);
        let err = g.validate().unwrap_err();
        assert!(err.contains("kind 'invalid_kind'"));
    }

    #[test]
    fn validate_accepts_http_and_https_references() {
        let mut entry = make_entry("entry-1", "Poly", "project");
        entry.references = vec![
            "https://example.com/poly".to_string(),
            "http://example.com/spec".to_string(),
        ];
        let g = make_glossary(vec![entry]);

        assert!(g.validate().is_ok());
    }

    #[test]
    fn validate_rejects_non_http_references() {
        let mut entry = make_entry("entry-1", "Poly", "project");
        entry.references = vec!["ftp://example.com/poly".to_string()];
        let g = make_glossary(vec![entry]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("must use http or https"));
    }

    #[test]
    fn validate_rejects_malformed_references() {
        let mut entry = make_entry("entry-1", "Poly", "project");
        entry.references = vec!["not a url".to_string()];
        let g = make_glossary(vec![entry]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("invalid URL"));
    }

    #[test]
    fn validate_rejects_secret_patterns() {
        let g = make_glossary(vec![make_entry("my_api_key", "SafeTerm", "other")]);
        let err = g.validate().unwrap_err();
        assert!(err.contains("secret"));
    }

    #[test]
    fn validate_rejects_secret_patterns_in_aliases() {
        let mut entry = make_entry("entry-1", "SafeTerm", "other");
        entry.aliases = vec!["my_password".to_string()];
        let g = make_glossary(vec![entry]);

        let err = g.validate().unwrap_err();

        assert!(err.contains("secret"));
        assert!(err.contains("password"));
    }

    #[test]
    fn normalize_trims_all_strings_and_references() {
        let mut entry = make_entry("  person-sujith  ", "  Sujith  ", "  person  ");
        entry.pronunciation = Some("  soo-jith  ".to_string());
        entry.aliases = vec!["  Suj  ".to_string(), "  ".to_string(), "".to_string()];
        entry.definition = Some("  A person  ".to_string());
        entry.notes = Some("  ".to_string());
        entry.references = vec!["  https://example.com/poly  ".to_string(), "".to_string()];
        let mut g = make_glossary(vec![entry]);

        g.normalize();

        let e = &g.entries[0];
        assert_eq!(e.id, "person-sujith");
        assert_eq!(e.term, "Sujith");
        assert_eq!(e.kind, "person");
        assert_eq!(e.pronunciation.as_deref(), Some("soo-jith"));
        assert_eq!(e.aliases, vec!["Suj"]);
        assert_eq!(e.definition.as_deref(), Some("A person"));
        assert_eq!(e.notes, None);
        assert_eq!(e.references, vec!["https://example.com/poly"]);
    }

    #[test]
    fn normalize_removes_empty_optionals() {
        let mut entry = make_entry("entry-1", "Test", "other");
        entry.pronunciation = Some("".to_string());
        entry.definition = Some("  ".to_string());
        entry.notes = Some("".to_string());
        let mut g = make_glossary(vec![entry]);

        g.normalize();

        let e = &g.entries[0];
        assert_eq!(e.pronunciation, None);
        assert_eq!(e.definition, None);
        assert_eq!(e.notes, None);
    }

    #[test]
    fn empty_glossary_produces_no_prompt_context() {
        let g = Glossary::default();
        assert!(g.to_prompt_context().is_empty());
    }

    #[test]
    fn serialize_empty_glossary_includes_empty_entries_vec() {
        let g = Glossary::default();
        let yaml = serde_yaml::to_string(&g).unwrap();
        assert!(yaml.contains("entries"));
        assert!(!yaml.contains("document_id"));
    }

    #[test]
    fn serialize_roundtrips_entry_id_and_references() {
        let mut entry = make_entry("entry-1", "Test", "other");
        entry.references = vec!["https://example.com/test".to_string()];
        let g = make_glossary(vec![entry]);

        let yaml = serde_yaml::to_string(&g).unwrap();
        let decoded: Glossary = serde_yaml::from_str(&yaml).unwrap();

        assert!(yaml.contains("id: entry-1"));
        assert!(yaml.contains("references:"));
        assert_eq!(decoded.entries[0].id, "entry-1");
        assert_eq!(decoded.entries[0].references, vec!["https://example.com/test"]);
    }

    #[test]
    fn serialize_skips_none_optionals() {
        let g = make_glossary(vec![make_entry("entry-1", "Test", "other")]);
        let yaml = serde_yaml::to_string(&g).unwrap();
        assert!(!yaml.contains("document_id"));
        assert!(!yaml.contains("pronunciation"));
        assert!(!yaml.contains("definition"));
        assert!(!yaml.contains("notes"));
    }
}
