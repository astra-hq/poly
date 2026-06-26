use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::config::ResourcefullyConfig;
use super::paths;

/// The service boundary for reading and writing [`ResourcefullyConfig`] from
/// YAML on disk.
///
/// All persistence goes through this repository rather than through the
/// bare `ResourcefullyConfig::save_to_path` / `ResourcefullyConfig::load_from_path`
/// convenience methods.  The repository owns the canonical config path and
/// provides an atomic write path that preserves the existing file on failure.
pub struct ConfigRepository {
    path: PathBuf,
}

impl ConfigRepository {
    /// Create a repository wired to the default config path
    /// (`~/.resourcefully/resourcefully.yml`).
    pub fn new() -> Self {
        Self {
            path: paths::default_config_path(),
        }
    }

    /// Create a repository wired to an explicit config path (useful for tests).
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Return the absolute path of the config file this repository manages.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ── load ──────────────────────────────────────────────────────────────

    /// Load the config from disk.
    ///
    /// Returns [`ResourcefullyConfig::default()`] if the file does not exist.
    pub fn load(&self) -> Result<ResourcefullyConfig> {
        ResourcefullyConfig::load_from_path(&self.path)
    }

    /// Load the config from disk, or create the default config and persist it
    /// atomically.
    ///
    /// This is the canonical "first-run" entry point: if no config file exists,
    /// the default is written to disk so subsequent loads find it.
    pub fn load_or_create_default(&self) -> Result<ResourcefullyConfig> {
        if self.path.exists() {
            self.load()
        } else {
            let default_cfg = ResourcefullyConfig::default();
            self.save_atomic(&default_cfg)?;
            Ok(default_cfg)
        }
    }

    // ── save (simple) ─────────────────────────────────────────────────────

    /// Non-atomic save: serialize to YAML and write directly to the config
    /// path.  Suitable for tests and simple cases where atomicity is not
    /// required.
    pub fn save(&self, config: &ResourcefullyConfig) -> Result<()> {
        paths::ensure_config_dir().with_context(|| "Failed to create config directory")?;

        let yaml =
            serde_yaml::to_string(config).with_context(|| "Failed to serialize config to YAML")?;

        std::fs::write(&self.path, yaml)
            .with_context(|| format!("Failed to write config to {}", self.path.display()))?;

        Ok(())
    }

    // ── save (atomic) ─────────────────────────────────────────────────────

    /// Atomically persist the config using temp-file + fsync + rename.
    ///
    /// The implementation:
    /// 1. Creates a temporary file in the same directory as the config file.
    /// 2. Serializes the config to YAML and writes it to the temp file.
    /// 3. Flushes and fsyncs the temp file so the data is durable on disk.
    /// 4. Atomically renames the temp file to the final config path.
    ///
    /// If **any** step fails, the original config file is left untouched and
    /// the temp file is cleaned up automatically.
    pub fn save_atomic(&self, config: &ResourcefullyConfig) -> Result<()> {
        // Ensure the parent directory exists so we can create the temp file.
        paths::ensure_config_dir().with_context(|| "Failed to create config directory")?;

        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        // Step 1 — temp file in the same directory (required for atomic rename).
        let mut temp_file = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("Failed to create temp file in {}", parent.display()))?;

        // Step 2 — serialize and write.
        let yaml =
            serde_yaml::to_string(config).with_context(|| "Failed to serialize config to YAML")?;

        temp_file
            .write_all(yaml.as_bytes())
            .with_context(|| "Failed to write YAML to temp file")?;

        // Step 3 — flush + fsync for durability.
        temp_file
            .flush()
            .with_context(|| "Failed to flush temp file")?;

        temp_file
            .as_file_mut()
            .sync_all()
            .with_context(|| "Failed to fsync temp file")?;

        // Step 4 — atomic rename.  On failure the NamedTempFile is dropped
        // and the original config file is preserved.
        temp_file.persist(&self.path).with_context(|| {
            format!(
                "Failed to atomically persist config to {}",
                self.path.display()
            )
        })?;

        Ok(())
    }
}

impl Default for ConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_default_picks_up_canonical_path() {
        let repo = ConfigRepository::new();
        assert!(repo.path().ends_with(".resourcefully/resourcefully.yml"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        let cfg = ResourcefullyConfig::default();
        repo.save(&cfg).unwrap();

        let loaded = repo.load().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.yml");
        let repo = ConfigRepository::with_path(file_path);

        let cfg = repo.load().unwrap();
        assert_eq!(cfg, ResourcefullyConfig::default());
    }

    #[test]
    fn load_or_create_default_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        // File does not exist yet.
        assert!(!file_path.exists());

        let cfg = repo.load_or_create_default().unwrap();
        assert_eq!(cfg, ResourcefullyConfig::default());

        // File was created.
        assert!(file_path.exists());

        // Subsequent load returns the same content.
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, ResourcefullyConfig::default());
    }

    #[test]
    fn save_atomic_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        let cfg = ResourcefullyConfig {
            summary: super::super::config::SummaryConfig {
                provider: "ollama".into(),
                model: "llama3.1:8b".into(),
                whisper_model: "medium".into(),
                ollama_endpoint: Some("http://localhost:11434".into()),
            },
            ..ResourcefullyConfig::default()
        };

        repo.save_atomic(&cfg).unwrap();

        assert!(file_path.exists());
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn save_atomic_preserves_original_on_readonly_directory() {
        use std::fs::{self, Permissions};
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        // Write an original config file first.
        let original_cfg = ResourcefullyConfig {
            summary: super::super::config::SummaryConfig {
                provider: "openai".into(),
                model: "original-model".into(),
                ..Default::default()
            },
            ..ResourcefullyConfig::default()
        };
        repo.save(&original_cfg).unwrap();

        let original_bytes = fs::read(&file_path).unwrap();

        // Make the directory read-only so creating a temp file inside it fails.
        // 0o555 = r-xr-xr-x (readable + traversable, not writable).
        let dir_path = dir.path().to_path_buf();
        fs::set_permissions(&dir_path, Permissions::from_mode(0o555)).unwrap();

        // Attempt an atomic save — it must fail.
        let new_cfg = ResourcefullyConfig {
            summary: super::super::config::SummaryConfig {
                provider: "claude".into(),
                model: "new-model".into(),
                ..Default::default()
            },
            ..ResourcefullyConfig::default()
        };
        let result = repo.save_atomic(&new_cfg);
        assert!(
            result.is_err(),
            "save_atomic must fail when directory is read-only"
        );

        // Restore permissions so tempdir can clean up on drop.
        let _ = fs::set_permissions(&dir_path, Permissions::from_mode(0o755));

        // Original file must be unchanged.
        let after_bytes = fs::read(&file_path).unwrap();
        assert_eq!(
            after_bytes, original_bytes,
            "Original config file must be preserved byte-for-byte after failed atomic save"
        );

        // Loaded config must still be the original.
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, original_cfg);
    }
}
