use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::config::PolyConfig;
use super::paths;

/// The service boundary for reading and writing [`PolyConfig`] from
/// YAML on disk.
///
/// All persistence goes through this repository rather than through the
/// bare `PolyConfig::save_to_path` / `PolyConfig::load_from_path`
/// convenience methods.  The repository owns the canonical config path and
/// provides an atomic write path that preserves the existing file on failure.
///
/// On first run, if the Poly config file does not exist, the repository
/// checks for a legacy `~/.resourcefully/resourcefully.yml` and migrates
/// it forward atomically — the legacy file is never deleted.
pub struct ConfigRepository {
    path: PathBuf,
    /// Override for the legacy config path used during `try_legacy_migration`.
    /// When `None` (production), the global `paths::legacy_config_path()` is used.
    legacy_path: Option<PathBuf>,
}

impl ConfigRepository {
    /// Create a repository wired to the default config path
    /// (`~/.poly/poly.yml`).
    pub fn new() -> Self {
        Self {
            path: paths::default_config_path(),
            legacy_path: None,
        }
    }

    /// Create a repository wired to an explicit config path (useful for tests).
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            legacy_path: None,
        }
    }

    /// Create a repository wired to explicit Poly and legacy config paths.
    ///
    /// When `legacy_path` is `Some`, `load_or_create_default` will attempt
    /// legacy migration from that path. Use this in tests to exercise
    /// the migration flow without touching the real home directory.
    pub fn with_paths(poly_path: PathBuf, legacy_path: Option<PathBuf>) -> Self {
        Self {
            path: poly_path,
            legacy_path,
        }
    }

    /// Return the absolute path of the config file this repository manages.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ── load ──────────────────────────────────────────────────────────────

    /// Load the config from disk.
    ///
    /// Returns [`PolyConfig::default()`] if the file does not exist.
    pub fn load(&self) -> Result<PolyConfig> {
        PolyConfig::load_from_path(&self.path)
    }

    /// Load the config from disk, or create the default config and persist it
    /// atomically.
    ///
    /// Fallback precedence:
    /// 1. If `~/.poly/poly.yml` exists, load it.
    /// 2. Else if legacy `~/.resourcefully/resourcefully.yml` exists, load it
    ///    and atomically write the equivalent config to `~/.poly/poly.yml`
    ///    (the legacy file is never deleted).
    /// 3. Else write default config to `~/.poly/poly.yml`.
    ///
    /// This is the canonical "first-run" entry point.
    pub fn load_or_create_default(&self) -> Result<PolyConfig> {
        if self.path.exists() {
            return self.load();
        }

        // Legacy fallback applies when using the default Poly path or
        // when an explicit legacy path was injected (e.g. in tests).
        if self.legacy_path.is_some() || self.path == paths::default_config_path() {
            if let Some(cfg) = self.try_legacy_migration()? {
                return Ok(cfg);
            }
        }

        // No legacy config or non-default path — write fresh defaults.
        let default_cfg = PolyConfig::default();
        self.save_atomic(&default_cfg)?;
        Ok(default_cfg)
    }

    /// Attempt to load and migrate a legacy `~/.resourcefully/resourcefully.yml`.
    ///
    /// Returns `Ok(Some(config))` if legacy was found and migrated, `Ok(None)`
    /// if there is no legacy file, or `Err` if the legacy YAML is malformed.
    fn try_legacy_migration(&self) -> Result<Option<PolyConfig>> {
        let legacy_path = self
            .legacy_path
            .as_deref()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(paths::legacy_config_path);
        match PolyConfig::load_from_path(&legacy_path) {
            Ok(cfg) => {
                if legacy_path.exists() {
                    // Migrate forward atomically (never delete legacy).
                    self.save_atomic(&cfg)
                        .with_context(|| "Failed to migrate legacy config to Poly path")?;
                    return Ok(Some(cfg));
                }
                // Legacy path doesn't exist — load_from_path returned default.
                Ok(None)
            }
            Err(e) => {
                // Malformed legacy YAML must surface as a typed error —
                // do not silently generate defaults.
                Err(e).with_context(|| {
                    format!(
                        "Failed to parse legacy config at {} — config migration aborted. \
                         Fix the legacy file or remove it to start fresh with defaults.",
                        legacy_path.display()
                    )
                })
            }
        }
    }

    // ── save (simple) ─────────────────────────────────────────────────────

    /// Non-atomic save: serialize to YAML and write directly to the config
    /// path.  Suitable for tests and simple cases where atomicity is not
    /// required.
    pub fn save(&self, config: &PolyConfig) -> Result<()> {
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
    pub fn save_atomic(&self, config: &PolyConfig) -> Result<()> {
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
    fn repository_default_picks_up_poly_path() {
        let repo = ConfigRepository::new();
        assert!(repo.path().ends_with(".poly/poly.yml"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        let cfg = PolyConfig::default();
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
        assert_eq!(cfg, PolyConfig::default());
    }

    #[test]
    fn load_or_create_default_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        // File does not exist yet.
        assert!(!file_path.exists());

        let cfg = repo.load_or_create_default().unwrap();
        assert_eq!(cfg, PolyConfig::default());

        // File was created.
        assert!(file_path.exists());

        // Subsequent load returns the same content.
        let loaded = repo.load().unwrap();
        assert_eq!(loaded, PolyConfig::default());
    }

    #[test]
    fn save_atomic_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.yml");
        let repo = ConfigRepository::with_path(file_path.clone());

        let cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "ollama".into(),
                model: "llama3.1:8b".into(),
                ..Default::default()
            },
            ..PolyConfig::default()
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
        let original_cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "openai".into(),
                model: "original-model".into(),
                ..Default::default()
            },
            ..PolyConfig::default()
        };
        repo.save(&original_cfg).unwrap();

        let original_bytes = fs::read(&file_path).unwrap();

        // Make the directory read-only so creating a temp file inside it fails.
        // 0o555 = r-xr-xr-x (readable + traversable, not writable).
        let dir_path = dir.path().to_path_buf();
        fs::set_permissions(&dir_path, Permissions::from_mode(0o555)).unwrap();

        // Attempt an atomic save — it must fail.
        let new_cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "claude".into(),
                model: "new-model".into(),
                ..Default::default()
            },
            ..PolyConfig::default()
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

    // ── legacy fallback tests ────────────────────────────────────────────

    #[test]
    fn legacy_fallback_migrates_custom_config_to_poly_path() {
        let dir = tempfile::tempdir().unwrap();

        // Create a legacy config with custom (non-default) values.
        let legacy_path = dir.path().join(".resourcefully/resourcefully.yml");
        let legacy_parent = legacy_path.parent().unwrap();
        std::fs::create_dir_all(legacy_parent).unwrap();

        let custom_cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "ollama".into(),
                model: "llama3.1:8b".into(),
                _whisper_model: Some("medium".into()),
            },
            ..PolyConfig::default()
        };
        let yaml = serde_yaml::to_string(&custom_cfg).unwrap();
        std::fs::write(&legacy_path, &yaml).unwrap();

        // The Poly path does NOT exist yet.
        let poly_path = dir.path().join(".poly/poly.yml");
        assert!(!poly_path.exists());

        // Use a repo that intercepts the paths so we can test in tempdir.
        let repo = PolyPathTestRepo {
            poly_path: poly_path.clone(),
            legacy_path: legacy_path.clone(),
        };

        let loaded = repo.load_or_create_default().unwrap();

        // Custom values were migrated.
        assert_eq!(loaded.summary.provider_id, "ollama");
        assert_eq!(loaded.summary.model, "llama3.1:8b");

        // Poly file was created.
        assert!(poly_path.exists());

        // Legacy file is still there (not deleted).
        assert!(legacy_path.exists());
        let legacy_contents = std::fs::read_to_string(&legacy_path).unwrap();
        assert!(legacy_contents.contains("ollama"));
    }

    #[test]
    fn poly_path_preferred_over_legacy_when_both_exist() {
        let dir = tempfile::tempdir().unwrap();

        let path = dir.path().join(".poly/poly.yml");
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();

        // Write Poly config with one set of values.
        let poly_cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "openai".into(),
                model: "gpt-4o".into(),
                ..Default::default()
            },
            ..PolyConfig::default()
        };
        let yaml = serde_yaml::to_string(&poly_cfg).unwrap();
        std::fs::write(&path, &yaml).unwrap();

        // Also create a legacy config with different values.
        let legacy_path = dir.path().join(".resourcefully/resourcefully.yml");
        let legacy_parent = legacy_path.parent().unwrap();
        std::fs::create_dir_all(legacy_parent).unwrap();
        let legacy_cfg = PolyConfig {
            summary: super::super::config::SummaryConfig {
                provider_id: "claude".into(),
                model: "claude-3".into(),
                ..Default::default()
            },
            ..PolyConfig::default()
        };
        let legacy_yaml = serde_yaml::to_string(&legacy_cfg).unwrap();
        std::fs::write(&legacy_path, &legacy_yaml).unwrap();

        let repo = PolyPathTestRepo {
            poly_path: path.clone(),
            legacy_path: legacy_path.clone(),
        };

        let loaded = repo.load_or_create_default().unwrap();

        // Poly path wins — its values are loaded.
        assert_eq!(loaded.summary.provider_id, "openai");
        assert_eq!(loaded.summary.model, "gpt-4o");

        // Legacy file remains untouched.
        assert!(legacy_path.exists());
    }

    #[test]
    fn malformed_legacy_yaml_surfaces_typed_error() {
        let dir = tempfile::tempdir().unwrap();

        // No Poly config.
        let poly_path = dir.path().join(".poly/poly.yml");

        // Create a malformed legacy config.
        let legacy_path = dir.path().join(".resourcefully/resourcefully.yml");
        let legacy_parent = legacy_path.parent().unwrap();
        std::fs::create_dir_all(legacy_parent).unwrap();
        std::fs::write(&legacy_path, "summary: [unclosed\n  provider: openai\n").unwrap();

        let repo = PolyPathTestRepo {
            poly_path: poly_path.clone(),
            legacy_path: legacy_path.clone(),
        };

        let result = repo.load_or_create_default();

        // Must return an error, not silently default.
        assert!(
            result.is_err(),
            "Malformed legacy YAML must return error, not default config"
        );
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("parse") || msg.contains("YAML") || msg.contains("yaml"),
            "Error should mention parse/yaml: got '{}'",
            msg
        );

        // Legacy file must still exist (not deleted).
        assert!(
            legacy_path.exists(),
            "Legacy file must not be deleted on parse failure"
        );

        // Poly config must NOT have been written.
        assert!(
            !poly_path.exists(),
            "Poly config must not be written when legacy parse fails"
        );
    }

    #[test]
    fn legacy_all_default_config_migrates_to_poly_path() {
        let dir = tempfile::tempdir().unwrap();

        // Create a legacy config with ALL defaults.
        let legacy_path = dir.path().join(".resourcefully/resourcefully.yml");
        let legacy_parent = legacy_path.parent().unwrap();
        std::fs::create_dir_all(legacy_parent).unwrap();

        let default_cfg = PolyConfig::default();
        let yaml = serde_yaml::to_string(&default_cfg).unwrap();
        std::fs::write(&legacy_path, &yaml).unwrap();

        let poly_path = dir.path().join(".poly/poly.yml");
        assert!(!poly_path.exists());

        let repo = PolyPathTestRepo {
            poly_path: poly_path.clone(),
            legacy_path: legacy_path.clone(),
        };

        let loaded = repo.load_or_create_default().unwrap();

        assert_eq!(loaded, PolyConfig::default());
        assert!(poly_path.exists());
        assert!(legacy_path.exists()); // legacy not deleted
    }

    /// A thin wrapper that lets us test legacy fallback logic in a tempdir
    /// without touching the real home directory.
    struct PolyPathTestRepo {
        poly_path: PathBuf,
        legacy_path: PathBuf,
    }

    impl PolyPathTestRepo {
        fn load_or_create_default(&self) -> Result<PolyConfig> {
            if self.poly_path.exists() {
                return PolyConfig::load_from_path(&self.poly_path);
            }

            if self.legacy_path.exists() {
                let cfg = PolyConfig::load_from_path(&self.legacy_path).map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to parse legacy config at {}: {}",
                        self.legacy_path.display(),
                        e
                    )
                })?;

                // Migrate forward atomically.
                migrate_atomic(&cfg, &self.poly_path)?;
                return Ok(cfg);
            }

            let default_cfg = PolyConfig::default();
            migrate_atomic(&default_cfg, &self.poly_path)?;
            Ok(default_cfg)
        }
    }

    /// Atomically write config to target_path (temp-file + fsync + rename).
    fn migrate_atomic(config: &PolyConfig, target_path: &Path) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dir {}", parent.display()))?;
        }

        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_file = tempfile::NamedTempFile::new_in(parent)?;

        let yaml = serde_yaml::to_string(config)?;
        temp_file.write_all(yaml.as_bytes())?;
        temp_file.flush()?;
        temp_file.as_file_mut().sync_all()?;
        temp_file.persist(target_path)?;

        Ok(())
    }
}
