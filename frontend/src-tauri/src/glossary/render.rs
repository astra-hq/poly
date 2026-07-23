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

fn render_references(entry: &GlossaryEntry) -> String {
    if entry.references.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n  References:");
    for reference in &entry.references {
        out.push_str(&format!("\n  - {}", reference));
    }
    out
}

fn push_kg_entry_fields(out: &mut String, entry: &GlossaryEntry) {
    out.push_str(&format!("- **ID:** {}\n", entry.id));
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
    if !entry.references.is_empty() {
        out.push_str("- **References:**\n");
        for reference in &entry.references {
            out.push_str(&format!("  - {}\n", reference));
        }
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
            "- **{}** ({}){}{}:{}{}{}\n",
            entry.term,
            entry.kind,
            render_pronunciation(entry),
            render_aliases(entry),
            render_definition(entry),
            render_notes(entry),
            render_references(entry)
        ));
    }

    out
}

pub fn render_kg_entry_document(entry: &GlossaryEntry) -> String {
    let mut out = format!("# Glossary Entry: {}\n\n", entry.term);
    push_kg_entry_fields(&mut out, entry);
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
        push_kg_entry_fields(&mut out, entry);
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::types::GlossaryEntry;

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
    fn prompt_context_empty_for_empty_glossary() {
        let g = Glossary::default();
        assert_eq!(render_prompt_context(&g), "");
    }

    #[test]
    fn prompt_context_includes_term_and_kind() {
        let g = make_glossary(vec![make_entry("person-sujith", "Sujith", "person")]);
        let result = render_prompt_context(&g);
        assert!(result.contains("**Sujith**"));
        assert!(result.contains("(person)"));
    }

    #[test]
    fn prompt_context_includes_pronunciation_when_present() {
        let mut entry = make_entry("person-sujith", "Sujith", "person");
        entry.pronunciation = Some("soo-jith".to_string());
        let g = make_glossary(vec![entry]);
        let result = render_prompt_context(&g);
        assert!(result.contains("(soo-jith)"));
    }

    #[test]
    fn prompt_context_includes_definition_and_notes() {
        let mut entry = make_entry("project-poly", "Poly", "project");
        entry.definition = Some("Privacy-first AI meeting assistant".to_string());
        entry.notes = Some("Formerly Meetily".to_string());
        let g = make_glossary(vec![entry]);
        let result = render_prompt_context(&g);
        assert!(result.contains("Privacy-first"));
        assert!(result.contains("Formerly Meetily"));
    }

    #[test]
    fn prompt_context_includes_aliases_when_present() {
        let mut entry = make_entry("project-parakeet", "Parakeet", "project");
        entry.pronunciation = Some("PAIR-uh-keet".to_string());
        entry.aliases = vec!["PK".to_string(), "parrot".to_string()];
        entry.definition = Some("ONNX-based fast transcription model".to_string());
        let g = make_glossary(vec![entry]);
        let result = render_prompt_context(&g);
        assert!(result.contains("[Aliases: PK, parrot]"));
        assert!(result.contains("(PAIR-uh-keet)"));
        assert!(result.contains("ONNX-based"));
    }

    #[test]
    fn prompt_context_omits_aliases_when_empty() {
        let g = make_glossary(vec![make_entry("other-minimal", "Minimal", "other")]);
        let result = render_prompt_context(&g);
        assert!(!result.contains("Aliases"));
    }

    #[test]
    fn prompt_context_renders_references_as_bullets() {
        let mut entry = make_entry("project-poly", "Poly", "project");
        entry.references = vec![
            "https://example.com/poly".to_string(),
            "https://example.com/spec".to_string(),
        ];
        let g = make_glossary(vec![entry]);

        let result = render_prompt_context(&g);

        assert!(result.contains("References:"));
        assert!(result.contains("  - https://example.com/poly"));
        assert!(result.contains("  - https://example.com/spec"));
    }

    #[test]
    fn kg_document_empty_glossary_shows_message() {
        let g = Glossary::default();
        let result = render_kg_document(&g);
        assert!(result.contains("No glossary entries"));
    }

    #[test]
    fn kg_document_includes_all_fields() {
        let mut entry = make_entry("project-parakeet", "Parakeet", "project");
        entry.pronunciation = Some("PAIR-uh-keet".to_string());
        entry.aliases = vec!["PK".to_string(), "parrot".to_string()];
        entry.definition = Some("ONNX-based fast transcription model".to_string());
        entry.notes = Some("Developed by NVIDIA".to_string());
        entry.references = vec!["https://example.com/parakeet".to_string()];
        let g = make_glossary(vec![entry]);
        let result = render_kg_document(&g);
        assert!(result.contains("# Glossary"));
        assert!(result.contains("**Version:** 1"));
        assert!(result.contains("## Parakeet"));
        assert!(result.contains("**ID:** project-parakeet"));
        assert!(result.contains("**Kind:** project"));
        assert!(result.contains("**Pronunciation:** PAIR-uh-keet"));
        assert!(result.contains("**Aliases:** PK, parrot"));
        assert!(result.contains("**Definition:** ONNX-based"));
        assert!(result.contains("**Notes:** Developed by NVIDIA"));
        assert!(result.contains("  - https://example.com/parakeet"));
    }

    #[test]
    fn kg_entry_document_renders_single_entry_references_as_bullets() {
        let mut entry = make_entry("project-poly", "Poly", "project");
        entry.definition = Some("Privacy-first AI meeting assistant".to_string());
        entry.references = vec![
            "https://example.com/poly".to_string(),
            "https://example.com/spec".to_string(),
        ];

        let result = render_kg_entry_document(&entry);

        assert!(result.contains("# Glossary Entry: Poly"));
        assert!(result.contains("**ID:** project-poly"));
        assert!(result.contains("- **References:**"));
        assert!(result.contains("  - https://example.com/poly"));
        assert!(result.contains("  - https://example.com/spec"));
    }

    #[test]
    fn kg_document_omits_optional_fields_when_none() {
        let g = make_glossary(vec![make_entry("other-minimal", "Minimal", "other")]);
        let result = render_kg_document(&g);
        assert!(!result.contains("Pronunciation"));
        assert!(!result.contains("Aliases"));
        assert!(!result.contains("Definition"));
        assert!(!result.contains("Notes"));
        assert!(!result.contains("References"));
    }
}
