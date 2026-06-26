//! Legacy-to-Poly app-data migration.
//!
//! When Poly's app data directory is empty or missing, this module
//! scans known legacy locations and copies forward existing data.
//! Legacy files are never deleted.
//!
//! # Items covered
//! - Databases: `meeting_minutes.sqlite`, `meeting_minutes.db`,
//!   associated WAL/SHM files
//! - Tauri store JSON: `recording_preferences.json`
//! - Models: `models/` (summary, whisper, parakeet subdirs)
//! - Templates: `templates/` (custom summary templates)
//! - Media/recordings: from `~/.resourcefully/media` to
//!   `~/.poly/media`
//!
//! # Safety
//! - Existing Poly data wins — migration only runs when Poly is
//!   empty.
//! - Files are copied, never moved or deleted from legacy
//!   locations.
//! - Errors are logged but do not block app startup.

use std::fs;
use std::path::{Path, PathBuf};

use log;

use super::paths::legacy_app_data_candidates;

/// Result from one migration attempt.
#[derive(Debug)]
pub struct MigrationResult {
    pub legacy_source: PathBuf,
    pub items_copied: Vec<String>,
    pub errors: Vec<String>,
}

/// Run the full legacy-to-Poly migration if needed.
///
/// `poly_app_data_dir` is Tauri's current `app.data_dir()` for Poly
/// (`com.poly.ai`). Returns true if a database was successfully
/// copied forward (meaning DB init can skip fresh creation).
pub fn migrate_if_needed(poly_app_data_dir: &Path) -> bool {
    // If Poly already has a database, do nothing.
    let sqlite_path = poly_app_data_dir.join("meeting_minutes.sqlite");
    let db_path = poly_app_data_dir.join("meeting_minutes.db");
    if sqlite_path.exists() || db_path.exists() {
        log::info!(
            "Poly app data already exists at {:?}, skipping legacy migration",
            poly_app_data_dir
        );
        return false;
    }

    log::info!(
        "Poly app data is empty ({:?}), searching for legacy data...",
        poly_app_data_dir
    );

    // Ensure Poly dir exists
    if let Err(e) = fs::create_dir_all(poly_app_data_dir) {
        log::error!("Failed to create Poly app data dir: {}", e);
        return false;
    }

    let candidates = legacy_app_data_candidates();
    log::info!("Legacy candidate dirs: {:?}", candidates);

    let mut db_copied = false;

    for candidate in &candidates {
        if !candidate.exists() {
            continue;
        }

        log::info!("Checking legacy directory: {:?}", candidate);

        let result = migrate_from_dir(candidate, poly_app_data_dir);
        if !result.items_copied.is_empty() {
            log::info!(
                "Migrated {} item(s) from {:?}: {:?}",
                result.items_copied.len(),
                candidate,
                result.items_copied
            );
        }
        if result.items_copied.iter().any(|s| s.contains("meeting_minutes")) {
            db_copied = true;
        }
        for err in &result.errors {
            log::warn!("Migration error from {:?}: {}", candidate, err);
        }
    }

    // Also handle legacy ~/.resourcefully/media -> ~/.poly/media
    migrate_legacy_media();

    db_copied
}

/// Copy app data from a single legacy directory into the Poly dir.
fn migrate_from_dir(legacy_dir: &Path, poly_dir: &Path) -> MigrationResult {
    let mut result = MigrationResult {
        legacy_source: legacy_dir.to_path_buf(),
        items_copied: Vec::new(),
        errors: Vec::new(),
    };

    // --- Database files ---
    for db_name in &[
        "meeting_minutes.sqlite",
        "meeting_minutes.db",
    ] {
        let src = legacy_dir.join(db_name);
        let dst = poly_dir.join(db_name);
        if src.exists() && !dst.exists() {
            match try_copy(&src, &dst) {
                Ok(_) => result.items_copied.push(db_name.to_string()),
                Err(e) => result.errors.push(format!("copy {} ({}) : {}", db_name, src.display(), e)),
            }
        }
    }

    // WAL / SHM files (only copy alongside matching db)
    for suffix in &["sqlite-wal", "sqlite-shm"] {
        let wal_name = format!("meeting_minutes.{}", suffix);
        let src = legacy_dir.join(&wal_name);
        let dst = poly_dir.join(&wal_name);
        if src.exists() && !dst.exists() {
            match try_copy(&src, &dst) {
                Ok(_) => result.items_copied.push(wal_name),
                Err(e) => result.errors.push(format!("copy WAL: {}", e)),
            }
        }
    }

    // --- Tauri store JSON ---
    let store_name = "recording_preferences.json";
    let src_store = legacy_dir.join(store_name);
    let dst_store = poly_dir.join(store_name);
    if src_store.exists() && !dst_store.exists() {
        match try_copy(&src_store, &dst_store) {
            Ok(_) => result.items_copied.push(store_name.to_string()),
            Err(e) => result.errors.push(format!("copy store: {}", e)),
        }
    }

    // --- Models directory ---
    let src_models = legacy_dir.join("models");
    let dst_models = poly_dir.join("models");
    if src_models.exists() && src_models.is_dir() && !dst_models.exists() {
        match copy_dir_recursive(&src_models, &dst_models) {
            Ok(files) => {
                let count = files.len();
                result.items_copied.push(format!("models/ ({} files)", count));
            }
            Err(e) => result.errors.push(format!("copy models: {}", e)),
        }
    }

    // --- Templates directory ---
    let src_templates = legacy_dir.join("templates");
    let dst_templates = poly_dir.join("templates");
    if src_templates.exists() && src_templates.is_dir() && !dst_templates.exists() {
        match copy_dir_recursive(&src_templates, &dst_templates) {
            Ok(files) => {
                let count = files.len();
                result.items_copied.push(format!("templates/ ({} files)", count));
            }
            Err(e) => result.errors.push(format!("copy templates: {}", e)),
        }
    }

    result
}

/// Migrate legacy `~/.resourcefully/media` recordings to
/// `~/.poly/media` if the legacy dir exists and Poly media is empty.
fn migrate_legacy_media() {
    use super::paths::{legacy_media_dir, poly_media_dir};

    let legacy = legacy_media_dir();
    let poly = poly_media_dir();

    if !legacy.exists() || !legacy.is_dir() {
        return;
    }
    if poly.exists() {
        log::info!(
            "Poly media dir already exists at {:?}, skipping media migration",
            poly
        );
        return;
    }

    // Create parent Poly dir
    if let Some(parent) = poly.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!("Failed to create Poly media parent dir: {}", e);
            return;
        }
    }

    match copy_dir_recursive(&legacy, &poly) {
        Ok(files) => log::info!("Migrated legacy media: {} files", files.len()),
        Err(e) => log::warn!("Failed to migrate legacy media: {}", e),
    }
}

/// Copy a single file, creating parent directories as needed.
fn try_copy(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir parent {:?}: {}", parent, e))?;
    }
    fs::copy(src, dst).map_err(|e| format!("fs::copy {:?} -> {:?}: {}", src, dst, e))?;
    Ok(())
}

/// Recursively copy a directory, returning the list of copied files.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<Vec<PathBuf>, String> {
    if !src.is_dir() {
        return Err(format!("{:?} is not a directory", src));
    }

    let mut copied = Vec::new();
    fs::create_dir_all(dst)
        .map_err(|e| format!("mkdir {:?}: {}", dst, e))?;

    let entries =
        fs::read_dir(src).map_err(|e| format!("read_dir {:?}: {}", src, e))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| format!("dir entry error: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            let sub = copy_dir_recursive(&src_path, &dst_path)?;
            copied.extend(sub);
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!("copy {:?} -> {:?}: {}", src_path, dst_path, e)
            })?;
            copied.push(dst_path);
        }
    }

    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_dirs() -> (TempDir, TempDir) {
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
    }

    #[test]
    fn empty_poly_with_empty_legacy_copies_nothing() {
        let (legacy, poly) = setup_dirs();
        // Both empty
        let copied = migrate_from_dir(legacy.path(), poly.path());
        assert!(copied.items_copied.is_empty());
        assert!(copied.errors.is_empty());
    }

    #[test]
    fn copies_meeting_minutes_db() {
        let (legacy, poly) = setup_dirs();
        let db_path = legacy.path().join("meeting_minutes.db");
        fs::write(&db_path, b"legacy data").unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        assert!(result.items_copied.contains(&"meeting_minutes.db".to_string()));
        let poly_db = poly.path().join("meeting_minutes.db");
        assert!(poly_db.exists());
        assert_eq!(fs::read_to_string(&poly_db).unwrap(), "legacy data");
        // Legacy still exists
        assert!(db_path.exists());
    }

    #[test]
    fn copies_meeting_minutes_sqlite() {
        let (legacy, poly) = setup_dirs();
        let db_path = legacy.path().join("meeting_minutes.sqlite");
        fs::write(&db_path, b"sqlite data").unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        assert!(result.items_copied.contains(&"meeting_minutes.sqlite".to_string()));
        assert!(poly.path().join("meeting_minutes.sqlite").exists());
        assert!(db_path.exists()); // legacy preserved
    }

    #[test]
    fn copies_recording_preferences_json() {
        let (legacy, poly) = setup_dirs();
        let pref_path = legacy.path().join("recording_preferences.json");
        fs::write(&pref_path, r#"{"auto_save":true}"#).unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        assert!(result.items_copied.contains(&"recording_preferences.json".to_string()));
        assert!(poly.path().join("recording_preferences.json").exists());
        assert!(pref_path.exists()); // legacy preserved
    }

    #[test]
    fn copies_models_directory() {
        let (legacy, poly) = setup_dirs();
        let models_dir = legacy.path().join("models").join("whisper");
        fs::create_dir_all(&models_dir).unwrap();
        fs::write(models_dir.join("model.bin"), b"model data").unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        let has_models = result
            .items_copied
            .iter()
            .any(|s| s.starts_with("models/"));
        assert!(has_models, "models should be copied, got: {:?}", result.items_copied);
        assert!(poly.path().join("models").join("whisper").join("model.bin").exists());
        assert!(models_dir.join("model.bin").exists()); // legacy preserved
    }

    #[test]
    fn copies_templates_directory() {
        let (legacy, poly) = setup_dirs();
        let templates_dir = legacy.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();
        fs::write(templates_dir.join("daily_standup.json"), r#"{"name":"standup"}"#).unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        let has_templates = result
            .items_copied
            .iter()
            .any(|s| s.starts_with("templates/"));
        assert!(has_templates, "templates should be copied");
        assert!(poly.path().join("templates").join("daily_standup.json").exists());
        assert!(templates_dir.join("daily_standup.json").exists()); // legacy preserved
    }

    #[test]
    fn existing_poly_data_not_overwritten() {
        let (legacy, poly) = setup_dirs();

        // Write legacy data
        fs::write(legacy.path().join("meeting_minutes.db"), b"old").unwrap();

        // Pre-existing Poly data
        fs::write(poly.path().join("meeting_minutes.db"), b"NEW POLY").unwrap();

        let result = migrate_from_dir(legacy.path(), poly.path());
        assert!(result.items_copied.is_empty(), "should not overwrite existing Poly data");
        assert_eq!(
            fs::read_to_string(poly.path().join("meeting_minutes.db")).unwrap(),
            "NEW POLY"
        );
    }

    #[test]
    fn migrate_if_needed_skips_when_poly_has_sqlite() {
        let poly_dir = tempfile::tempdir().unwrap();
        fs::write(poly_dir.path().join("meeting_minutes.sqlite"), b"data").unwrap();

        let copied = migrate_if_needed(poly_dir.path());
        assert!(!copied);
    }

    #[test]
    fn migrate_if_needed_skips_when_poly_has_db() {
        let poly_dir = tempfile::tempdir().unwrap();
        fs::write(poly_dir.path().join("meeting_minutes.db"), b"data").unwrap();

        let copied = migrate_if_needed(poly_dir.path());
        assert!(!copied);
    }

    #[test]
    fn legacy_files_preserved_after_migration() {
        let (legacy, poly) = setup_dirs();
        let db_path = legacy.path().join("meeting_minutes.db");
        fs::write(&db_path, b"data").unwrap();

        migrate_from_dir(legacy.path(), poly.path());

        // Legacy file must still exist
        assert!(db_path.exists(), "legacy file must NOT be deleted");
    }
}
