use super::types::{Glossary, GlossaryEntry};

fn render_pronunciation(entry: &GlossaryEntry) -> String {
    match &entry.pronunciation {
        Some(pron) => format!(" ({})", pron),
        None => String::new(),
    }
}

fn render_definition(entry: &GlossaryEntry) -> String {
    match &entry.definition {
        Some(def) => format!("\n  Definition: {}", def),
        None => String::new(),
    }
}

fn render_aliases(entry: &GlossaryEntry) -> String {
    if entry.aliases.is_empty() {
        String::new()
    } else {
        format!(" [Aliases: {}]", entry.aliases.join(", "))
    }
}

fn render_notes(entry: &GlossaryEntry) -> String {
    match &entry.notes {
        Some(n) => format!("\n  Notes: {}", n),
        None => String::new(),
    }
}

pub fn render_prompt_context(glossary: &Glossary) -> String {
    if glossary.entries.is_empty() {
        return String::new();
    }

    let mut out = String::from("## Glossary\n\n");
    out.push_str("The following terms are relevant to this meeting:\n\n");

    for entry in &glossary.entries {
        out.push_str(&format!(
            "- **{}** ({}){}{}:{}{}\n",
            entry.term,
            entry.kind,
            render_pronunciation(entry),
            render_aliases(entry),
            render_definition(entry),
            render_notes(entry)
        ));
    }

    out
}

pub fn render_kg_document(glossary: &Glossary) -> String {
    let mut out = String::from("# Glossary\n\n");

    if glossary.entries.is_empty() {
        out.push_str("_No glossary entries defined._\n");
        return out;
    }

    out.push_str(&format!("**Version:** {}\n\n", glossary.version));

    for entry in &glossary.entries {
        out.push_str(&format!("## {}\n\n", entry.term));
        out.push_str(&format!("- **Kind:** {}\n", entry.kind));

        if let Some(pron) = &entry.pronunciation {
            out.push_str(&format!("- **Pronunciation:** {}\n", pron));
        }
        if !entry.aliases.is_empty() {
            out.push_str(&format!("- **Aliases:** {}\n", entry.aliases.join(", ")));
        }
        if let Some(def) = &entry.definition {
            out.push_str(&format!("- **Definition:** {}\n", def));
        }
        if let Some(n) = &entry.notes {
            out.push_str(&format!("- **Notes:** {}\n", n));
        }

        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::types::GlossaryEntry;

    #[test]
    fn prompt_context_empty_for_empty_glossary() {
        let g = Glossary::default();
        assert_eq!(render_prompt_context(&g), "");
    }

    #[test]
    fn prompt_context_includes_term_and_kind() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Sujith".to_string(),
                kind: "person".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        let result = render_prompt_context(&g);
        assert!(result.contains("**Sujith**"));
        assert!(result.contains("(person)"));
    }

    #[test]
    fn prompt_context_includes_pronunciation_when_present() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Sujith".to_string(),
                kind: "person".to_string(),
                pronunciation: Some("soo-jith".to_string()),
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        let result = render_prompt_context(&g);
        assert!(result.contains("(soo-jith)"));
    }

    #[test]
    fn prompt_context_includes_definition_and_notes() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Poly".to_string(),
                kind: "project".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: Some("Privacy-first AI meeting assistant".to_string()),
                notes: Some("Formerly Meetily".to_string()),
            }],
        };
        let result = render_prompt_context(&g);
        assert!(result.contains("Privacy-first"));
        assert!(result.contains("Formerly Meetily"));
    }

    #[test]
    fn prompt_context_includes_aliases_when_present() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Parakeet".to_string(),
                kind: "project".to_string(),
                pronunciation: Some("PAIR-uh-keet".to_string()),
                aliases: vec!["PK".to_string(), "parrot".to_string()],
                definition: Some("ONNX-based fast transcription model".to_string()),
                notes: None,
            }],
        };
        let result = render_prompt_context(&g);
        assert!(result.contains("[Aliases: PK, parrot]"));
        assert!(result.contains("(PAIR-uh-keet)"));
        assert!(result.contains("ONNX-based"));
    }

    #[test]
    fn prompt_context_omits_aliases_when_empty() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Minimal".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        let result = render_prompt_context(&g);
        assert!(!result.contains("Aliases"));
    }

    #[test]
    fn kg_document_empty_glossary_shows_message() {
        let g = Glossary::default();
        let result = render_kg_document(&g);
        assert!(result.contains("No glossary entries"));
    }

    #[test]
    fn kg_document_includes_all_fields() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Parakeet".to_string(),
                kind: "project".to_string(),
                pronunciation: Some("PAIR-uh-keet".to_string()),
                aliases: vec!["PK".to_string(), "parrot".to_string()],
                definition: Some("ONNX-based fast transcription model".to_string()),
                notes: Some("Developed by NVIDIA".to_string()),
            }],
        };
        let result = render_kg_document(&g);
        assert!(result.contains("# Glossary"));
        assert!(result.contains("**Version:** 1"));
        assert!(result.contains("## Parakeet"));
        assert!(result.contains("**Kind:** project"));
        assert!(result.contains("**Pronunciation:** PAIR-uh-keet"));
        assert!(result.contains("**Aliases:** PK, parrot"));
        assert!(result.contains("**Definition:** ONNX-based"));
        assert!(result.contains("**Notes:** Developed by NVIDIA"));
    }

    #[test]
    fn kg_document_omits_optional_fields_when_none() {
        let g = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Minimal".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        let result = render_kg_document(&g);
        assert!(!result.contains("Pronunciation"));
        assert!(!result.contains("Aliases"));
        assert!(!result.contains("Definition"));
        assert!(!result.contains("Notes"));
    }
}
