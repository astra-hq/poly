use async_trait::async_trait;

use super::store::SecretStore;
use super::types::{SecretRef, SecretStoreError};

/// OS keychain-backed secret store.
///
/// Uses the `keyring` crate to store secrets in the platform-native
/// credential manager:
/// - macOS: Keychain
/// - Windows: Credential Manager
/// - Linux: Secret Service (org.freedesktop.secrets) or similar
///
/// Secrets are stored under a fixed service name (`com.meetily.secrets`)
/// with the secret reference namespace as the account/username field.
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// Create a new keyring store with the given service name.
    ///
    /// The default service name is `"com.meetily.secrets"`.
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new("com.meetily.secrets")
    }
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn get(
        &self,
        key: &SecretRef,
    ) -> Result<Option<String>, SecretStoreError> {
        let entry = keyring::Entry::new(&self.service, key.as_str())
            .map_err(|e| {
                SecretStoreError::KeyringError(format!(
                    "failed to create keyring entry: {}",
                    e
                ))
            })?;

        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => {
                // Treat any other keyring error (including unavailable backend)
                // as "not found" so callers get a graceful fallback.
                let msg = format!("keyring unavailable: {}", e);
                log::debug!("{}", msg);
                Ok(None)
            }
        }
    }

    async fn set(
        &self,
        key: &SecretRef,
        value: &str,
    ) -> Result<(), SecretStoreError> {
        let entry = keyring::Entry::new(&self.service, key.as_str())
            .map_err(|e| {
                SecretStoreError::KeyringError(format!(
                    "failed to create keyring entry: {}",
                    e
                ))
            })?;

        entry.set_password(value).map_err(|e| {
            SecretStoreError::KeyringError(format!(
                "failed to write to keyring: {}",
                e
            ))
        })
    }

    async fn delete(
        &self,
        key: &SecretRef,
    ) -> Result<(), SecretStoreError> {
        let entry = keyring::Entry::new(&self.service, key.as_str())
            .map_err(|e| {
                SecretStoreError::KeyringError(format!(
                    "failed to create keyring entry: {}",
                    e
                ))
            })?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // already gone
            Err(e) => Err(SecretStoreError::KeyringError(format!(
                "failed to delete from keyring: {}",
                e
            ))),
        }
    }

    async fn exists(
        &self,
        key: &SecretRef,
    ) -> Result<bool, SecretStoreError> {
        self.get(key).await.map(|v| v.is_some())
    }
}

#[cfg(test)]
#[cfg(target_os = "macos")] // Keychain tests require macOS keychain access
mod tests {
    use super::*;

    fn test_store() -> KeyringSecretStore {
        KeyringSecretStore::new("com.meetily.secrets.test")
    }

    fn cleanup(key: &SecretRef) {
        let store = test_store();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(store.delete(key));
    }

    #[test]
    fn set_and_get_roundtrip() {
        let store = test_store();
        let key = SecretRef::new("keyring-test/set-get").unwrap();
        cleanup(&key);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(store.set(&key, "test-secret-123")).unwrap();

        let val = rt.block_on(store.get(&key)).unwrap();
        assert_eq!(val.as_deref(), Some("test-secret-123"));

        cleanup(&key);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = test_store();
        let key = SecretRef::new("keyring-test/nonexistent").unwrap();
        cleanup(&key);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let val = rt.block_on(store.get(&key)).unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn delete_nonexistent_is_noop() {
        let store = test_store();
        let key = SecretRef::new("keyring-test/noop-delete").unwrap();
        cleanup(&key);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(store.delete(&key));
        assert!(result.is_ok());
    }
}
