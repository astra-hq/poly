use serde::{Deserialize, Serialize};
use std::fmt;

/// Validated wrapper around a secret namespace string.
///
/// Namespace format: `component/subcomponent/path/key`
///
/// Allowed characters: alphanumeric, underscores, hyphens, forward slashes.
/// The namespace is validated on construction and never re-validated internally.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SecretRef(String);

impl SecretRef {
    /// Create a validated `SecretRef` from a namespace string.
    ///
    /// Returns an error if the namespace is empty or contains invalid characters.
    pub fn new(ns: impl Into<String>) -> Result<Self, SecretStoreError> {
        let ns = ns.into();
        if ns.is_empty() {
            return Err(SecretStoreError::InvalidNamespace(
                "namespace cannot be empty".into(),
            ));
        }
        if !ns
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/')
        {
            return Err(SecretStoreError::InvalidNamespace(format!(
                "namespace contains invalid characters: {}",
                ns
            )));
        }
        Ok(Self(ns))
    }

    /// Create a `SecretRef` without validation.
    ///
    /// Prefer `SecretRef::new` in public APIs. This is for internal use
    /// where the namespace is guaranteed valid at compile time.
    pub(crate) fn new_unchecked(ns: impl Into<String>) -> Self {
        Self(ns.into())
    }

    /// Return the inner namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&SecretRef> for String {
    fn from(r: &SecretRef) -> String {
        r.0.clone()
    }
}

/// Errors that can occur during secret store operations.
///
/// Each variant carries a human-readable message. The `IoError`
/// variant implements `From<std::io::Error>` for ergonomic `?` use.
#[derive(Debug)]
pub enum SecretStoreError {
    /// The namespace string is invalid (empty or contains disallowed characters).
    InvalidNamespace(String),
    /// A generic store-level failure (persistence, lookup, deserialization).
    StoreError(String),
    /// The OS keychain returned an error.
    KeyringError(String),
    /// Serialization or deserialization of the secrets file failed.
    SerializationError(String),
    /// An I/O error occurred during file operations.
    IoError(std::io::Error),
}

impl fmt::Display for SecretStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespace(msg) => write!(f, "invalid namespace: {}", msg),
            Self::StoreError(msg) => write!(f, "store error: {}", msg),
            Self::KeyringError(msg) => write!(f, "keyring error: {}", msg),
            Self::SerializationError(msg) => write!(f, "serialization error: {}", msg),
            Self::IoError(e) => write!(f, "io error: {}", e),
        }
    }
}

impl From<std::io::Error> for SecretStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_namespace() {
        let r = SecretRef::new("provider/summary/openai/api_key").unwrap();
        assert_eq!(r.as_str(), "provider/summary/openai/api_key");
    }

    #[test]
    fn valid_namespace_with_hyphens_underscores() {
        let r = SecretRef::new("provider/summary/custom-openai/api_key").unwrap();
        assert_eq!(r.as_str(), "provider/summary/custom-openai/api_key");
    }

    #[test]
    fn empty_namespace_is_rejected() {
        let err = SecretRef::new("").unwrap_err();
        match err {
            SecretStoreError::InvalidNamespace(_) => {}
            _ => panic!("expected InvalidNamespace, got {:?}", err),
        }
    }

    #[test]
    fn namespace_with_spaces_is_rejected() {
        assert!(SecretRef::new("provider / summary").is_err());
    }

    #[test]
    fn namespace_with_special_chars_is_rejected() {
        assert!(SecretRef::new("provider@summary").is_err());
    }

    #[test]
    fn display_returns_namespace() {
        let r = SecretRef::new("foo/bar").unwrap();
        assert_eq!(format!("{}", r), "foo/bar");
    }

    #[test]
    fn serialization_roundtrip() {
        let r = SecretRef::new("provider/summary/openai/api_key").unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let restored: SecretRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, restored);
    }

    #[test]
    fn equality_based_on_namespace() {
        let a = SecretRef::new("foo/bar").unwrap();
        let b = SecretRef::new("foo/bar").unwrap();
        let c = SecretRef::new("baz/qux").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
