use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::Glossary;
use crate::poly_config::paths;

#[derive(Clone)]
pub struct GlossaryRepository {
    path: PathBuf,
}

impl GlossaryRepository {
    pub fn new() -> Self {
        Self {
            path: paths::glossary_path(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Glossary> {
        if !self.path.exists() {
            return Ok(Glossary::default());
        }

        let contents = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read glossary from {}", self.path.display()))?;

        if contents.trim().is_empty() {
            return Ok(Glossary::default());
        }

        let glossary: Glossary = serde_yaml::from_str(&contents)
            .with_context(|| format!("Malformed YAML in glossary file {}", self.path.display()))?;

        Ok(glossary)
    }

    pub fn save(&self, glossary: &Glossary) -> Result<()> {
        paths::ensure_glossary_dir()
            .with_context(|| "Failed to create glossary directory")?;

        let yaml = serde_yaml::to_string(glossary)
            .with_context(|| "Failed to serialize glossary to YAML")?;

        std::fs::write(&self.path, yaml)
            .with_context(|| format!("Failed to write glossary to {}", self.path.display()))?;

        Ok(())
    }

    pub fn save_atomic(&self, glossary: &Glossary) -> Result<()> {
        paths::ensure_glossary_dir()
            .with_context(|| "Failed to create glossary directory")?;

        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        let mut temp_file = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("Failed to create temp file in {}", parent.display()))?;

        let yaml = serde_yaml::to_string(glossary)
            .with_context(|| "Failed to serialize glossary to YAML")?;

        temp_file
            .write_all(yaml.as_bytes())
            .with_context(|| "Failed to write YAML to temp file")?;

        temp_file
            .flush()
            .with_context(|| "Failed to flush temp file")?;

        temp_file
            .as_file_mut()
            .sync_all()
            .with_context(|| "Failed to fsync temp file")?;

        temp_file.persist(&self.path).with_context(|| {
            format!(
                "Failed to atomically persist glossary to {}",
                self.path.display()
            )
        })?;

        Ok(())
    }
}

impl Default for GlossaryRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glossary::types::GlossaryEntry;

    #[test]
    fn repository_default_picks_up_glossary_path() {
        let repo = GlossaryRepository::new();
        assert!(repo.path().ends_with(".poly/glossary.yml"));
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.yml");
        let repo = GlossaryRepository::with_path(file_path);

        let glossary = repo.load().unwrap();
        assert_eq!(glossary, Glossary::default());
    }

    #[test]
    fn load_empty_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.yml");
        std::fs::write(&file_path, "").unwrap();
        let repo = GlossaryRepository::with_path(file_path);

        let glossary = repo.load().unwrap();
        assert_eq!(glossary, Glossary::default());
    }

    #[test]
    fn load_whitespace_only_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("whitespace.yml");
        std::fs::write(&file_path, "   \n  \n  ").unwrap();
        let repo = GlossaryRepository::with_path(file_path);

        let glossary = repo.load().unwrap();
        assert_eq!(glossary, Glossary::default());
    }

    #[test]
    fn load_malformed_yaml_returns_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("malformed.yml");
        std::fs::write(&file_path, "entries: [unclosed\n  - !!bad").unwrap();
        let repo = GlossaryRepository::with_path(file_path);

        let result = repo.load();
        assert!(result.is_err(), "Malformed YAML must return an error");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Malformed YAML") || err.contains("yaml") || err.contains("YAML"),
            "Error should mention YAML parsing: got '{}'",
            err
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let glossary = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Sujith".to_string(),
                kind: "person".to_string(),
                pronunciation: Some("soo-jith".to_string()),
                aliases: vec!["Suj".to_string()],
                definition: Some("Project lead".to_string()),
                notes: Some("Based in SF".to_string()),
            }],
        };

        repo.save(&glossary).unwrap();

        assert!(file_path.exists());
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, glossary);
    }

    #[test]
    fn save_atomic_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let glossary = Glossary {
            version: 1,
            entries: vec![
                GlossaryEntry {
                    term: "Poly".to_string(),
                    kind: "project".to_string(),
                    pronunciation: None,
                    aliases: vec![],
                    definition: Some("Privacy-first AI meeting assistant".to_string()),
                    notes: None,
                },
                GlossaryEntry {
                    term: "Parakeet".to_string(),
                    kind: "project".to_string(),
                    pronunciation: Some("PAIR-uh-keet".to_string()),
                    aliases: vec!["PK".to_string()],
                    definition: Some("ONNX-based fast transcription".to_string()),
                    notes: None,
                },
            ],
        };

        repo.save_atomic(&glossary).unwrap();

        assert!(file_path.exists());
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, glossary);
    }

    #[test]
    fn save_atomic_preserves_original_on_failure() {
        use std::fs::{self, Permissions};
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let original = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Original".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        repo.save(&original).unwrap();
        let original_bytes = fs::read(&file_path).unwrap();

        let dir_path = dir.path().to_path_buf();
        fs::set_permissions(&dir_path, Permissions::from_mode(0o555)).unwrap();

        let new_glossary = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "New".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        let result = repo.save_atomic(&new_glossary);
        assert!(
            result.is_err(),
            "save_atomic must fail when directory is read-only"
        );

        let _ = fs::set_permissions(&dir_path, Permissions::from_mode(0o755));

        let after_bytes = fs::read(&file_path).unwrap();
        assert_eq!(after_bytes, original_bytes);

        let loaded = repo.load().unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn save_atomic_does_not_cache_stale_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let first = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "First".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        repo.save_atomic(&first).unwrap();

        let second = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "Second".to_string(),
                kind: "other".to_string(),
                pronunciation: None,
                aliases: vec![],
                definition: None,
                notes: None,
            }],
        };
        repo.save_atomic(&second).unwrap();

        let loaded = repo.load().unwrap();
        assert_eq!(loaded, second);
        assert_ne!(loaded, first);
    }

    #[test]
    fn save_load_roundtrip_preserves_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("glossary.yml");
        let repo = GlossaryRepository::with_path(file_path.clone());

        let glossary = Glossary {
            version: 1,
            entries: vec![GlossaryEntry {
                term: "FullEntry".to_string(),
                kind: "component".to_string(),
                pronunciation: Some("full-EN-tree".to_string()),
                aliases: vec!["FE".to_string(), "Full".to_string()],
                definition: Some("A complete entry for testing".to_string()),
                notes: Some("Created during roundtrip test".to_string()),
            }],
        };

        repo.save_atomic(&glossary).unwrap();

        assert!(file_path.exists());
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, glossary);

        let file_content = std::fs::read_to_string(&file_path).unwrap();
        assert!(file_content.contains("FullEntry"));
        assert!(file_content.contains("component"));
        assert!(file_content.contains("full-EN-tree"));
    }
}
