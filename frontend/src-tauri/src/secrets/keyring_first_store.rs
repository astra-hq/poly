use std::path::PathBuf;

use async_trait::async_trait;

use super::file_store::FileSecretStore;
use super::keyring_store::KeyringSecretStore;
use super::store::SecretStore;
use super::types::{SecretRef, SecretStoreError};

/// Composite secret store that tries the OS keychain first and falls back
/// to a file-based store.
///
/// # Strategy
///
/// | Operation | Keychain result | File result | Outcome |
/// |-----------|----------------|-------------|---------|
/// | `get`     | `Ok(Some(v))`  | (skipped)   | `Ok(Some(v))` |
/// | `get`     | `Ok(None)`     | `Ok(v)`     | `Ok(v)` |
/// | `get`     | `Err`          | `Ok(v)`     | `Ok(v)` |
/// | `set`     | (always calls) | (always calls) | `Ok(())` if either succeeds |
/// | `delete`  | `Ok(())`       | `file.delete` | `Ok(())` (file deletion is authoritative) |
/// | `delete`  | `Err`          | `file.delete` | file's result |
pub struct KeyringFirstSecretStore {
    keyring: KeyringSecretStore,
    file_store: FileSecretStore,
}

impl KeyringFirstSecretStore {
    /// Create a new composite store.
    ///
    /// - `keyring_service`: the service name for the OS keychain (e.g.,
    ///   `"com.meetily.secrets"`).
    /// - `file_path`: the path to the fallback YAML secrets file.
    pub fn new(
        keyring_service: impl Into<String>,
        file_path: PathBuf,
    ) -> Self {
        Self {
            keyring: KeyringSecretStore::new(keyring_service),
            file_store: FileSecretStore::new(file_path),
        }
    }

    /// Create a store with the canonical defaults:
    /// - Keychain service: `"com.meetily.secrets"`
    /// - File path: `~/.resourcefully/secrets.yml`
    pub fn default_store() -> Result<Self, SecretStoreError> {
        let home = dirs::home_dir().ok_or_else(|| {
            SecretStoreError::StoreError(
                "cannot determine home directory".into(),
            )
        })?;
        let file_path = home.join(".resourcefully").join("secrets.yml");
        Ok(Self::new("com.meetily.secrets", file_path))
    }
}

#[async_trait]
impl SecretStore for KeyringFirstSecretStore {
    async fn get(
        &self,
        key: &SecretRef,
    ) -> Result<Option<String>, SecretStoreError> {
        match self.keyring.get(key).await {
            Ok(Some(val)) => Ok(Some(val)),
            Ok(None) | Err(_) => self.file_store.get(key).await,
        }
    }

    async fn set(
        &self,
        key: &SecretRef,
        value: &str,
    ) -> Result<(), SecretStoreError> {
        // Always persist to both stores. If keyring passes, that is the
        // primary store; the file mirror ensures the fallback get path works.
        let keyring_result = self.keyring.set(key, value).await;
        let file_result = self.file_store.set(key, value).await;
        keyring_result.or(file_result)
    }

    async fn delete(
        &self,
        key: &SecretRef,
    ) -> Result<(), SecretStoreError> {
        // Try keychain first; always also delete from file to avoid stale
        // fallback entries.
        let _ = self.keyring.delete(key).await;
        self.file_store.delete(key).await
    }

    async fn exists(
        &self,
        key: &SecretRef,
    ) -> Result<bool, SecretStoreError> {
        self.get(key).await.map(|v| v.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_composite_store() -> (KeyringFirstSecretStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.yml");
        let store = KeyringFirstSecretStore::new(
            "com.meetily.secrets.test.composite",
            path,
        );
        (store, dir)
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let (store, _dir) = temp_composite_store();
        let key = SecretRef::new("composite-test/nonexistent").unwrap();
        let val = store.get(&key).await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn set_and_get_roundtrip_via_file() {
        // Keyring may not be available in CI — this should still work
        // because the file fallback kicks in.
        let (store, _dir) = temp_composite_store();
        let key = SecretRef::new("composite-test/roundtrip").unwrap();
        store.set(&key, "file-backed-secret").await.unwrap();
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("file-backed-secret"));
    }

    #[tokio::test]
    async fn delete_removes_from_file() {
        let (store, _dir) = temp_composite_store();
        let key = SecretRef::new("composite-test/delete").unwrap();
        store.set(&key, "temp").await.unwrap();
        store.delete(&key).await.unwrap();
        assert!(!store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_correctly() {
        let (store, _dir) = temp_composite_store();
        let key = SecretRef::new("composite-test/exists-check").unwrap();
        assert!(!store.exists(&key).await.unwrap());

        store.set(&key, "val").await.unwrap();
        assert!(store.exists(&key).await.unwrap());

        store.delete(&key).await.unwrap();
        assert!(!store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn default_store_constructor_succeeds() {
        let result = KeyringFirstSecretStore::default_store();
        assert!(result.is_ok());
    }
}
