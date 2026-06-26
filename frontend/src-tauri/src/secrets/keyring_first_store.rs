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
///
/// # Migration-aware legacy fallback
///
/// When `legacy_keyring` or `legacy_file_store` are set (as in
/// `default_store()`), `get` probes legacy stores after the primary stores
/// and copies any discovered value into the Poly primary stores
/// (copy-forward migration). `set` and `delete` only touch the primary
/// stores, never the legacy stores.
pub struct KeyringFirstSecretStore {
    keyring: KeyringSecretStore,
    file_store: FileSecretStore,
    /// Legacy keychain store (com.meetily.secrets) for migration read.
    legacy_keyring: Option<KeyringSecretStore>,
    /// Legacy file store (~/.resourcefully/secrets.yml) for migration read.
    legacy_file_store: Option<FileSecretStore>,
}

impl KeyringFirstSecretStore {
    /// Create a new composite store.
    ///
    /// - `keyring_service`: the service name for the OS keychain (e.g.,
    ///   `"com.poly.secrets"`).
    /// - `file_path`: the path to the fallback YAML secrets file.
    pub fn new(keyring_service: impl Into<String>, file_path: PathBuf) -> Self {
        Self {
            keyring: KeyringSecretStore::new(keyring_service),
            file_store: FileSecretStore::new(file_path),
            legacy_keyring: None,
            legacy_file_store: None,
        }
    }

    /// Create a composite store with legacy fallback stores for migration.
    ///
    /// `get` will probe the legacy stores after the primary stores and
    /// copy-forward any discovered value. `set` and `delete` only touch
    /// the primary stores.
    pub fn with_legacy(
        keyring_service: impl Into<String>,
        file_path: PathBuf,
        legacy_keyring_service: impl Into<String>,
        legacy_file_path: PathBuf,
    ) -> Self {
        Self {
            keyring: KeyringSecretStore::new(keyring_service),
            file_store: FileSecretStore::new(file_path),
            legacy_keyring: Some(KeyringSecretStore::new(legacy_keyring_service)),
            legacy_file_store: Some(FileSecretStore::new(legacy_file_path)),
        }
    }

    /// Create a store with the canonical defaults:
    /// - Primary keychain: `"com.poly.secrets"`
    /// - Primary file: `~/.poly/secrets.yml`
    /// - Legacy keychain: `"com.meetily.secrets"` (migration fallback)
    /// - Legacy file: `~/.resourcefully/secrets.yml` (migration fallback)
    pub fn default_store() -> Result<Self, SecretStoreError> {
        let home = dirs::home_dir().ok_or_else(|| {
            SecretStoreError::StoreError("cannot determine home directory".into())
        })?;
        let file_path = home.join(".poly").join("secrets.yml");
        let legacy_file_path = home.join(".resourcefully").join("secrets.yml");
        Ok(Self::with_legacy(
            "com.poly.secrets",
            file_path,
            "com.meetily.secrets",
            legacy_file_path,
        ))
    }

    /// Try legacy stores for a key and copy-forward on success.
    async fn try_legacy(&self, key: &SecretRef) -> Option<String> {
        // Legacy keychain first
        if let Some(legacy_kr) = &self.legacy_keyring {
            if let Ok(Some(val)) = legacy_kr.get(key).await {
                self.copy_forward(key, &val).await;
                return Some(val);
            }
        }
        // Legacy file store
        if let Some(legacy_file) = &self.legacy_file_store {
            if let Ok(Some(val)) = legacy_file.get(key).await {
                self.copy_forward(key, &val).await;
                return Some(val);
            }
        }
        None
    }

    /// Copy a value discovered in a legacy store forward into Poly primary.
    async fn copy_forward(&self, key: &SecretRef, value: &str) {
        self.set(key, value).await.ok();
    }
}

#[async_trait]
impl SecretStore for KeyringFirstSecretStore {
    async fn get(&self, key: &SecretRef) -> Result<Option<String>, SecretStoreError> {
        // Primary keychain first
        match self.keyring.get(key).await {
            Ok(Some(val)) => return Ok(Some(val)),
            // Keychain miss: try primary file
            Ok(None) | Err(_) => {}
        }
        // Primary file store
        if let Ok(Some(val)) = self.file_store.get(key).await {
            return Ok(Some(val));
        }
        // Legacy stores with copy-forward
        Ok(self.try_legacy(key).await)
    }

    async fn set(&self, key: &SecretRef, value: &str) -> Result<(), SecretStoreError> {
        // Always persist to both primary stores. If keyring passes, that is the
        // primary store; the file mirror ensures the fallback get path works.
        // Legacy stores are never touched by `set`.
        let keyring_result = self.keyring.set(key, value).await;
        let file_result = self.file_store.set(key, value).await;
        keyring_result.or(file_result)
    }

    async fn delete(&self, key: &SecretRef) -> Result<(), SecretStoreError> {
        // Delete from primary stores only; legacy entries are preserved.
        let _ = self.keyring.delete(key).await;
        self.file_store.delete(key).await
    }

    async fn exists(&self, key: &SecretRef) -> Result<bool, SecretStoreError> {
        self.get(key).await.map(|v| v.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_composite_store() -> (KeyringFirstSecretStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.yml");
        let store = KeyringFirstSecretStore::new("com.poly.secrets.test.composite", path);
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

    // ── Migration-aware tests ────────────────────────────────────────

    /// Creates a store with legacy fallback using temp directories.
    fn temp_migration_store(
        legacy_file_path: PathBuf,
    ) -> (KeyringFirstSecretStore, PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let poly_path = dir.path().join("poly_secrets.yml");
        let store = KeyringFirstSecretStore::with_legacy(
            "com.poly.secrets.test.migration",
            poly_path.clone(),
            "com.meetily.secrets.test.legacy",
            legacy_file_path,
        );
        (store, poly_path, dir)
    }

    #[tokio::test]
    async fn get_from_legacy_file_fallback() {
        // Pre-populate a legacy file store
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/legacy-key").unwrap();
        legacy_store.set(&key, "legacy-value").await.unwrap();

        // Create migration-aware store with the legacy file
        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

        // Read should find it in legacy
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("legacy-value"));
    }

    #[tokio::test]
    async fn legacy_read_copies_forward_to_poly() {
        // Pre-populate a legacy file store
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/copy-fwd").unwrap();
        legacy_store.set(&key, "pre-migration-value").await.unwrap();

        // Create migration-aware store
        let (store, poly_path, _dir) = temp_migration_store(legacy_path.clone());

        // First read — triggers copy-forward
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("pre-migration-value"));

        // Now the Poly primary file should have the value (copy-forward worked)
        let poly_store = FileSecretStore::new(poly_path);
        let poly_val = poly_store.get(&key).await.unwrap();
        assert_eq!(
            poly_val.as_deref(),
            Some("pre-migration-value"),
            "value should have been copied forward to Poly primary store"
        );

        // Subsequent read from composite should hit Poly primary directly
        let val2 = store.get(&key).await.unwrap();
        assert_eq!(val2.as_deref(), Some("pre-migration-value"));
    }

    #[tokio::test]
    async fn legacy_store_not_deleted_on_poly_delete() {
        // Pre-populate a legacy file store
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/dont-delete").unwrap();
        legacy_store.set(&key, "keep-me").await.unwrap();

        // Create migration-aware store
        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

        // Delete through primary store
        store.delete(&key).await.unwrap();

        // Legacy should still have the value
        let legacy_val = legacy_store.get(&key).await.unwrap();
        assert_eq!(
            legacy_val.as_deref(),
            Some("keep-me"),
            "legacy entry must not be deleted"
        );
    }

    #[tokio::test]
    async fn poly_primary_wins_over_legacy() {
        // Populate legacy with one value and primary with another
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/primary-wins").unwrap();
        legacy_store.set(&key, "legacy-value").await.unwrap();

        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());
        // Write directly to Poly primary
        store.set(&key, "poly-value").await.unwrap();

        // Read should return Poly value, not legacy
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("poly-value"));
    }

    #[tokio::test]
    async fn set_does_not_touch_legacy_store() {
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/set-primary-only").unwrap();
        // Legacy has nothing for this key

        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

        // Set a value through the migration-aware store
        store.set(&key, "poly-only-val").await.unwrap();

        // Legacy store should still not have this key
        let legacy_val = legacy_store.get(&key).await.unwrap();
        assert!(legacy_val.is_none(), "set must not write to legacy store");

        // But primary should have it
        let val = store.get(&key).await.unwrap();
        assert_eq!(val.as_deref(), Some("poly-only-val"));
    }

    #[tokio::test]
    async fn missing_secret_returns_none_with_legacy_stores() {
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());
        let key = SecretRef::new("migration/truly-missing").unwrap();

        let val = store.get(&key).await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn exists_with_legacy_fallback() {
        let legacy_dir = tempfile::tempdir().unwrap();
        let legacy_path = legacy_dir.path().join("secrets.yml");
        let legacy_store = FileSecretStore::new(legacy_path.clone());
        let key = SecretRef::new("migration/exists-legacy").unwrap();

        // Not in either store
        let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());
        assert!(!store.exists(&key).await.unwrap());

        // Populate legacy
        legacy_store.set(&key, "exists-val").await.unwrap();

        // Now exists should be true (found in legacy)
        assert!(store.exists(&key).await.unwrap());

        // After copy-forward, should still exist (in primary now)
        assert!(store.exists(&key).await.unwrap());
    }
}
