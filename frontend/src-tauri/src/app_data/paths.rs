//! Poly app data paths and legacy path resolution.
//!
//! Provides current (Poly) paths and enumerates legacy candidates used
//! during app-data migration. Path constants exist only for migration;
//! the authoritative Poly app data directory is always obtained from
//! Tauri's runtime `app.path().app_data_dir()`.

use std::path::PathBuf;

/// Returns the current (Poly) default media/recordings directory:
/// `~/.poly/media`.
pub fn poly_media_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".poly").join("media")
}

/// Returns the legacy Resourcefully media/recordings directory:
/// `~/.resourcefully/media`. Used for fallback only; never written.
pub fn legacy_media_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".resourcefully").join("media")
}

/// Returns the old `dirs::data_dir()`-based Meetily directory.
/// Example: `~/Library/Application Support/Meetily/` on macOS.
pub fn legacy_meetily_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Meetily"))
}

/// Returns the legacy Tauri app data directory for the old
/// `com.meetily.ai` bundle identifier. This is a best-effort
/// reconstruction of what Tauri's `app_data_dir` would have
/// returned for the old identifier.
pub fn legacy_meetily_app_data_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    Some(data_dir.join("com.meetily.ai"))
}

/// List all candidate legacy app-data directories to search
/// during migration, in priority order (most specific first).
pub fn legacy_app_data_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Old Tauri app data dir under com.meetily.ai
    if let Some(d) = legacy_meetily_app_data_dir() {
        candidates.push(d);
    }

    // 2. Generic Meetily data dir (used by model managers)
    if let Some(d) = legacy_meetily_data_dir() {
        if !candidates.contains(&d) {
            candidates.push(d);
        }
    }

    // 3. Legacy Resourcefully media dir
    let legacy_media = legacy_media_dir();
    candidates.push(legacy_media);

    // 4. Homebrew paths (macOS)
    #[cfg(target_os = "macos")]
    {
        let homebrew_apple_silicon = PathBuf::from("/opt/homebrew/var/meetily");
        let homebrew_intel = PathBuf::from("/usr/local/var/meetily");
        candidates.push(homebrew_apple_silicon);
        candidates.push(homebrew_intel);
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poly_media_dir_ends_in_poly_media() {
        let path = poly_media_dir();
        assert!(path.ends_with(".poly/media"));
    }

    #[test]
    fn legacy_media_dir_ends_in_resourcefully_media() {
        let path = legacy_media_dir();
        assert!(path.ends_with(".resourcefully/media"));
    }

    #[test]
    fn legacy_candidates_not_empty() {
        let candidates = legacy_app_data_candidates();
        // At minimum we always have the legacy resourcefully media dir
        assert!(candidates.len() >= 1);
    }
}
