use log::info;
use tauri::{AppHandle, Runtime};

use crate::glossary::{Glossary, GlossaryRepository};
use crate::state::AppState;

async fn load_glossary(repo: &GlossaryRepository) -> Result<Glossary, String> {
    repo.load()
        .map_err(|e| format!("Failed to load glossary: {}", e))
}

async fn save_glossary(
    repo: &GlossaryRepository,
    mut glossary: Glossary,
) -> Result<Glossary, String> {
    glossary.normalize();
    glossary.validate()?;
    repo.save_atomic(&glossary)
        .map_err(|e| format!("Failed to save glossary: {}", e))?;
    Ok(glossary)
}

#[tauri::command]
pub async fn api_get_glossary<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
) -> Result<Glossary, String> {
    info!("api_get_glossary called");
    let repo = GlossaryRepository::new();
    load_glossary(&repo).await
}

#[tauri::command]
pub async fn api_save_glossary<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    glossary: Glossary,
) -> Result<Glossary, String> {
    info!(
        "api_save_glossary called ({} entries)",
        glossary.entries.len()
    );
    let repo = GlossaryRepository::new();
    save_glossary(&repo, glossary).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::types::GlossaryEntry;
    use std::fs;

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

    #[tokio::test]
    async fn get_missing_file_returns_empty_glossary() {
        let dir = tempfile::tempdir().unwrap();
        let repo = GlossaryRepository::with_path(dir.path().join("glossary.yml"));

        let glossary = load_glossary(&repo).await.unwrap();

        assert_eq!(glossary, Glossary::default());
    }

    #[tokio::test]
    async fn save_normalizes_aliases_and_optional_strings() {
        let dir = tempfile::tempdir().unwrap();
        let repo = GlossaryRepository::with_path(dir.path().join("glossary.yml"));
        let glossary = make_glossary(vec![GlossaryEntry {
            id: "person-sujith".to_string(),
            term: "  Sujith  ".to_string(),
            kind: "person".to_string(),
            pronunciation: Some("  soo-jith  ".to_string()),
            aliases: vec!["  Suj  ".to_string(), "  ".to_string(), "".to_string()],
            definition: Some("  A person  ".to_string()),
            notes: Some("  ".to_string()),
            references: vec!["  https://example.com/sujith  ".to_string()],
        }]);

        let saved = save_glossary(&repo, glossary).await.unwrap();

        assert_eq!(saved.entries[0].term, "Sujith");
        assert_eq!(saved.entries[0].kind, "person");
        assert_eq!(saved.entries[0].pronunciation.as_deref(), Some("soo-jith"));
        assert_eq!(saved.entries[0].aliases, vec!["Suj"]);
        assert_eq!(saved.entries[0].definition.as_deref(), Some("A person"));
        assert_eq!(saved.entries[0].notes, None);
        assert_eq!(saved.entries[0].references, vec!["https://example.com/sujith"]);

        let persisted = repo.load().unwrap();
        assert_eq!(persisted, saved);
    }

    #[tokio::test]
    async fn invalid_glossary_returns_error_and_preserves_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let original = make_glossary(vec![make_entry("entry-original", "Original", "other")]);
        repo.save_atomic(&original).unwrap();
        let original_bytes = fs::read(&file_path).unwrap();

        let invalid = make_glossary(vec![
            make_entry("entry-1", "Duplicate", "person"),
            make_entry("entry-2", "duplicate", "team"),
        ]);

        let err = save_glossary(&repo, invalid).await.unwrap_err();
        assert!(err.contains("duplicate"));

        let current_bytes = fs::read(&file_path).unwrap();
        assert_eq!(current_bytes, original_bytes);
        assert_eq!(repo.load().unwrap(), original);
    }
}
