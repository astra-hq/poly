use std::path::PathBuf;

/// Returns the current (Poly) config file path: `~/.poly/poly.yml`
pub fn default_config_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".poly").join("poly.yml")
}

/// Returns the legacy Resourcefully config file path:
/// `~/.resourcefully/resourcefully.yml`.
///
/// This path is used only for migration/fallback — it is never
/// written to or deleted by the current app.
pub fn legacy_config_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".resourcefully").join("resourcefully.yml")
}

/// Ensures the parent directory for the current (Poly) config file exists.
pub fn ensure_config_dir() -> std::io::Result<()> {
    let path = default_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Ensures the parent directory for the legacy config file exists.
/// Used only during fallback migration — the app does not rely on
/// this directory otherwise.
pub fn ensure_legacy_config_dir() -> std::io::Result<()> {
    let path = legacy_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_path_ends_in_poly_yml() {
        let path = default_config_path();
        assert!(path.ends_with(".poly/poly.yml"));
    }

    #[test]
    fn legacy_config_path_ends_in_resourcefully_yml() {
        let path = legacy_config_path();
        assert!(path.ends_with(".resourcefully/resourcefully.yml"));
    }

    #[test]
    fn ensure_config_dir_does_not_panic() {
        let result = ensure_config_dir();
        assert!(result.is_ok());
    }
}
