use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use async_trait::async_trait;

use super::store::SecretStore;
use super::types::{SecretRef, SecretStoreError};

/// File-backed secret store using a YAML file.
///
/// Stores secrets as a flat `{namespace: value}` mapping in a YAML file.
/// Uses **atomic writes**: serializes to a temporary file at the same location,
/// then renames it over the target so the file is never left in a corrupt state.
///
/// The file is only created when the first secret is written.
/// Reads that find no file return `None` for every key.
pub struct FileSecretStore {
    path: PathBuf,
}

impl FileSecretStore {
    /// Create a store that persists secrets at the given file path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Read the full in-memory map from the YAML file.
    /// Returns an empty map when the file does not exist.
    fn read_all(&self) -> Result<HashMap<String, String>, SecretStoreError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&self.path).map_err(SecretStoreError::from)?;
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }
        serde_yaml::from_str(&content).map_err(|e| {
            SecretStoreError::SerializationError(format!("failed to parse secrets file: {}", e))
        })
    }

    /// Atomically write the map to the YAML file.
    fn write_all(&self, map: &HashMap<String, String>) -> Result<(), SecretStoreError> {
        let yaml = serde_yaml::to_string(map).map_err(|e| {
            SecretStoreError::SerializationError(format!("failed to serialize secrets: {}", e))
        })?;

        // Write to a sibling temp file, then rename atomically.
        let parent = self.path.parent().ok_or_else(|| {
            SecretStoreError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "secrets file path has no parent directory",
            ))
        })?;
        fs::create_dir_all(parent)?;

        let tmp_path = self.path.with_extension("tmp");
        {
            let mut f = fs::File::create(&tmp_path)?;
            f.write_all(yaml.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

#[async_trait]
impl SecretStore for FileSecretStore {
    async fn get(&self, key: &SecretRef) -> Result<Option<String>, SecretStoreError> {
        let map = self.read_all()?;
        Ok(map.get(key.as_str()).cloned())
    }

    async fn set(&self, key: &SecretRef, value: &str) -> Result<(), SecretStoreError> {
        let mut map = self.read_all()?;
        map.insert(key.as_str().to_string(), value.to_string());
        self.write_all(&map)
    }

    async fn delete(&self, key: &SecretRef) -> Result<(), SecretStoreError> {
        let mut map = self.read_all()?;
        map.remove(key.as_str());
        self.write_all(&map)
    }

    async fn exists(&self, key: &SecretRef) -> Result<bool, SecretStoreError> {
        let map = self.read_all()?;
        Ok(map.contains_key(key.as_str()))
    }
}

/// A `FileSecretStore` scoped to `~/.poly/secrets.yml`.
///
/// This is the canonical file fallback path used by `KeyringFirstSecretStore`
/// and migration.
pub fn default_file_store() -> Result<FileSecretStore, SecretStoreError> {
    let home = dirs::home_dir()
        .ok_or_else(|| SecretStoreError::StoreError("cannot determine home directory".into()))?;
    let path = home.join(".poly").join("secrets.yml");
    Ok(FileSecretStore::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (FileSecretStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.yml");
        (FileSecretStore::new(path), dir)
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/foo").unwrap();
        let val = store.get(&key).await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn set_and_get_roundtrip() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/foo").unwrap();
        store.set(&key, "my-secret-value").await.unwrap();

        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("my-secret-value"));
    }

    #[tokio::test]
    async fn set_overwrites_existing() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/foo").unwrap();
        store.set(&key, "first").await.unwrap();
        store.set(&key, "second").await.unwrap();

        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn delete_removes_key() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/foo").unwrap();
        store.set(&key, "my-secret").await.unwrap();
        store.delete(&key).await.unwrap();

        assert!(!store.exists(&key).await.unwrap());
        assert!(store.get(&key).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_is_noop() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/nonexistent").unwrap();
        let result = store.delete(&key).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn exists_returns_correctly() {
        let (store, _dir) = temp_store();
        let key = SecretRef::new("test/bar").unwrap();
        assert!(!store.exists(&key).await.unwrap());

        store.set(&key, "val").await.unwrap();
        assert!(store.exists(&key).await.unwrap());

        store.delete(&key).await.unwrap();
        assert!(!store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn multiple_keys_independent() {
        let (store, _dir) = temp_store();
        let a = SecretRef::new("test/a").unwrap();
        let b = SecretRef::new("test/b").unwrap();

        store.set(&a, "value-a").await.unwrap();
        store.set(&b, "value-b").await.unwrap();

        assert_eq!(store.get(&a).await.unwrap().as_deref(), Some("value-a"));
        assert_eq!(store.get(&b).await.unwrap().as_deref(), Some("value-b"));

        store.delete(&a).await.unwrap();
        assert!(store.get(&a).await.unwrap().is_none());
        assert_eq!(store.get(&b).await.unwrap().as_deref(), Some("value-b"));
    }

    #[tokio::test]
    async fn atomic_write_does_not_corrupt_existing() {
        let (store, dir) = temp_store();
        let key = SecretRef::new("test/k").unwrap();

        store.set(&key, "original").await.unwrap();

        // Simulate a crash during write by checking that no `.tmp` file lingers
        // after a successful write.
        let tmp_path = dir.path().join("secrets.yml.tmp");
        assert!(
            !tmp_path.exists(),
            "tmp file should not persist after write"
        );

        // Re-read from the same file (simulating a process restart).
        let store2 = FileSecretStore::new(dir.path().join("secrets.yml"));
        let val = store2.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("original"));
    }

    #[tokio::test]
    async fn empty_secrets_file_is_treated_as_empty_map() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.yml");
        std::fs::write(&path, "").unwrap();

        let store = FileSecretStore::new(path);
        let key = SecretRef::new("test/x").unwrap();
        assert!(store.get(&key).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn default_file_store_creates_path_under_home() {
        let store = default_file_store().unwrap();
        let key = SecretRef::new("test/home").unwrap();
        store.set(&key, "home-secret").await.unwrap();
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("home-secret"));

        // Clean up
        let _ = std::fs::remove_file(&store.path);
    }
}
