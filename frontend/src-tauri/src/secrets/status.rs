use serde::{Deserialize, Serialize};

use super::store::SecretStore;
use super::types::{SecretRef, SecretStoreError};

/// Lightweight status for an API key stored in a [`SecretStore`].
///
/// Returned to the frontend so the UI can show whether a key is configured
/// without ever exposing the raw secret value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyStatus {
    /// Whether a non-empty secret exists for the given ref.
    #[serde(rename = "hasSecret")]
    pub has_secret: bool,
    /// The canonical namespace of this secret (e.g., `provider/summary/openai/api_key`).
    #[serde(rename = "secretRef")]
    pub secret_ref: String,
    /// A masked hint suitable for display: first 3 chars + "..." + last 4 chars.
    /// `None` when no secret is stored.
    #[serde(rename = "maskedHint", skip_serializing_if = "Option::is_none")]
    pub masked_hint: Option<String>,
}

/// Build an [`ApiKeyStatus`] by querying `store` for the given `secret_ref`.
pub async fn build_api_key_status(
    store: &dyn SecretStore,
    secret_ref: &SecretRef,
) -> Result<ApiKeyStatus, SecretStoreError> {
    let secret = store.get(secret_ref).await?;
    let has_secret = secret.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let masked_hint = secret.as_deref().filter(|s| !s.is_empty()).map(mask_api_key);
    Ok(ApiKeyStatus {
        has_secret,
        secret_ref: secret_ref.as_str().to_string(),
        masked_hint,
    })
}

/// Returns a display-safe masked key: first 3 characters + `...` + last 4
/// characters.  When the key is too short to split safely, returns `"..."`.
pub fn mask_api_key(key: &str) -> String {
    let len = key.chars().count();
    if len <= 7 {
        return "...".to_string();
    }
    let prefix: String = key.chars().take(3).collect();
    let suffix: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}...{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_typical_key() {
        let result = mask_api_key("sk-abcdefghijklmnop1234");
        // "sk-" + "..." + "1234" = "sk-...1234"
        assert_eq!(result, "sk-...1234");
    }

    #[test]
    fn mask_full_length_custom_openai() {
        let result = mask_api_key("sk-proj-1234567890abcdef1234567890abcdef");
        // first 3: "sk-", last 4: "cdef"
        assert_eq!(result, "sk-...cdef");
    }

    #[test]
    fn mask_exactly_seven_chars_returns_dots() {
        // len <= 7 → "..."
        assert_eq!(mask_api_key("1234567"), "...");
    }

    #[test]
    fn mask_eight_chars() {
        // "12345678" → first 3: "123", last 4: "5678"
        assert_eq!(mask_api_key("12345678"), "123...5678");
    }

    #[test]
    fn mask_empty_string() {
        assert_eq!(mask_api_key(""), "...");
    }

    #[test]
    fn mask_short_string() {
        assert_eq!(mask_api_key("ab"), "...");
    }

    #[test]
    fn mask_unicode_key() {
        // emoji + text, just verify it doesn't panic
        let _ = mask_api_key("🔑-my-secret-key-12345");
    }
}
