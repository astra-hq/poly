use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Glossary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<GlossaryEntry>,
}

fn default_version() -> u32 {
    1
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
        let mut seen_terms: HashSet<String> = HashSet::new();

        for (i, entry) in self.entries.iter().enumerate() {
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

            let all_text = format!(
                "{} {} {:?} {:?} {:?}",
                entry.term,
                entry.aliases.join(" "),
                entry.pronunciation,
                entry.definition,
                entry.notes
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

    fn make_entry(term: &str, kind: &str) -> GlossaryEntry {
        GlossaryEntry {
            term: term.to_string(),
            kind: kind.to_string(),
            pronunciation: None,
            aliases: vec![],
            definition: None,
            notes: None,
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
        let g = Glossary {
            version: 1,
            entries: vec![
                make_entry("Sujith", "person"),
                make_entry("Parakeet", "project"),
                make_entry("TLA", "acronym"),
            ],
        };
        assert!(g.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_term() {
        let g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "   ".to_string(),
                    kind: "person".to_string(),
                    pronunciation: None,
                    aliases: vec![],
                    definition: None,
                    notes: None,
                },
            ],
        };
        let err = g.validate().unwrap_err();
        assert!(err.contains("must be non-empty"));
    }

    #[test]
    fn validate_rejects_duplicate_terms_case_insensitive() {
        let g = Glossary {
            version: 1,
            entries: vec![
                make_entry("Sujith", "person"),
                make_entry("sujith", "person"),
            ],
        };
        let err = g.validate().unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn validate_rejects_invalid_kind() {
        let g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "test".to_string(),
                    kind: "invalid_kind".to_string(),
                    pronunciation: None,
                    aliases: vec![],
                    definition: None,
                    notes: None,
                },
            ],
        };
        let err = g.validate().unwrap_err();
        assert!(err.contains("kind 'invalid_kind'"));
    }

    #[test]
    fn validate_rejects_secret_patterns() {
        let g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "my_api_key".to_string(),
                    kind: "other".to_string(),
                    pronunciation: None,
                    aliases: vec![],
                    definition: None,
                    notes: None,
                },
            ],
        };
        let err = g.validate().unwrap_err();
        assert!(err.contains("secret"));
    }

    #[test]
    fn validate_rejects_secret_patterns_in_aliases() {
        let g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "SafeTerm".to_string(),
                    kind: "other".to_string(),
                    pronunciation: None,
                    aliases: vec!["my_password".to_string()],
                    definition: None,
                    notes: None,
                },
            ],
        };
        let err = g.validate().unwrap_err();
        assert!(err.contains("secret"));
        assert!(err.contains("password"));
    }

    #[test]
    fn normalize_trims_all_strings() {
        let mut g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "  Sujith  ".to_string(),
                    kind: "  person  ".to_string(),
                    pronunciation: Some("  soo-jith  ".to_string()),
                    aliases: vec!["  Suj  ".to_string(), "  ".to_string(), "".to_string()],
                    definition: Some("  A person  ".to_string()),
                    notes: Some("  ".to_string()),
                },
            ],
        };
        g.normalize();
        let e = &g.entries[0];
        assert_eq!(e.term, "Sujith");
        assert_eq!(e.kind, "person");
        assert_eq!(e.pronunciation, Some("soo-jith".to_string()));
        assert_eq!(e.aliases, vec!["Suj"]);
        assert_eq!(e.definition, Some("A person".to_string()));
        assert_eq!(e.notes, None);
    }

    #[test]
    fn normalize_removes_empty_optionals() {
        let mut g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "Test".to_string(),
                    kind: "other".to_string(),
                    pronunciation: Some("".to_string()),
                    aliases: vec![],
                    definition: Some("  ".to_string()),
                    notes: Some("".to_string()),
                },
            ],
        };
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
    fn serialize_empty_glossary_omits_empty_entries_vec() {
        let g = Glossary::default();
        let yaml = serde_yaml::to_string(&g).unwrap();
        assert!(!yaml.contains("entries"));
    }

    #[test]
    fn serialize_skips_none_optionals() {
        let g = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "Test".to_string(),
                    kind: "other".to_string(),
                    pronunciation: None,
                    aliases: vec![],
                    definition: None,
                    notes: None,
                },
            ],
        };
        let yaml = serde_yaml::to_string(&g).unwrap();
        assert!(!yaml.contains("pronunciation"));
        assert!(!yaml.contains("definition"));
        assert!(!yaml.contains("notes"));
    }
}
