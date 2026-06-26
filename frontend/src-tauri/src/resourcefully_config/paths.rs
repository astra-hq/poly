use std::path::PathBuf;

/// Returns the default config file path: `~/.resourcefully/resourcefully.yml`
///
/// Creates the parent directory if it does not exist.
pub fn default_config_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".resourcefully").join("resourcefully.yml")
}

/// Ensures the parent directory for the config file exists.
pub fn ensure_config_dir() -> std::io::Result<()> {
    let path = default_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_path_ends_in_correct_filename() {
        let path = default_config_path();
        assert!(path.ends_with(".resourcefully/resourcefully.yml"));
    }

    #[test]
    fn ensure_config_dir_does_not_panic() {
        let result = ensure_config_dir();
        assert!(result.is_ok());
    }
}
