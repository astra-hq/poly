use async_trait::async_trait;

use super::types::{SecretRef, SecretStoreError};

/// Async trait for secret storage backends.
///
/// Implementations must be `Send + Sync` so they can be shared across
/// Tauri command handlers. The trait is object-safe and can be used
/// with `Arc<dyn SecretStore>`.
///
/// # Design notes
///
/// - `set` overwrites an existing value silently (upsert semantics).
/// - `delete` of a non-existent key is a no-op, not an error.
/// - `get` returns `None` when the key does not exist.
/// - Raw secret values are never returned in error messages.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Retrieve a secret by reference. Returns `None` if not found.
    async fn get(&self, key: &SecretRef) -> Result<Option<String>, SecretStoreError>;

    /// Store a secret, overwriting any existing value.
    async fn set(&self, key: &SecretRef, value: &str) -> Result<(), SecretStoreError>;

    /// Delete a secret. No-op if the key does not exist.
    async fn delete(&self, key: &SecretRef) -> Result<(), SecretStoreError>;

    /// Check whether a secret exists.
    async fn exists(&self, key: &SecretRef) -> Result<bool, SecretStoreError>;
}
