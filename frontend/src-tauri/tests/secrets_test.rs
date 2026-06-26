/// Integration tests for the secrets module — exercises the public API
/// of FileSecretStore and KeyringFirstSecretStore.
use app_lib::secrets::file_store::FileSecretStore;
use app_lib::secrets::keyring_first_store::KeyringFirstSecretStore;
use app_lib::secrets::store::SecretStore;
use app_lib::secrets::types::SecretRef;

fn temp_file_store() -> (FileSecretStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.yml");
    (FileSecretStore::new(path), dir)
}

// ── FileSecretStore integration tests ──────────────────────────────

#[tokio::test]
async fn file_store_set_and_get_integration() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/provider/summary/openai").unwrap();

    store.set(&key, "sk-integration-test-key").await.unwrap();
    let got = store.get(&key).await.unwrap();

    assert_eq!(got.as_deref(), Some("sk-integration-test-key"));
}

#[tokio::test]
async fn file_store_get_missing_returns_none() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/missing/key").unwrap();

    let got = store.get(&key).await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn file_store_delete_removes_secret() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/to-delete").unwrap();

    store.set(&key, "temp").await.unwrap();
    assert!(store.exists(&key).await.unwrap());

    store.delete(&key).await.unwrap();
    assert!(!store.exists(&key).await.unwrap());
}

#[tokio::test]
async fn file_store_overwrite_updates_value() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/overwrite").unwrap();

    store.set(&key, "old").await.unwrap();
    store.set(&key, "new").await.unwrap();

    assert_eq!(store.get(&key).await.unwrap().as_deref(), Some("new"));
}

#[tokio::test]
async fn file_store_independent_keys() {
    let (store, _dir) = temp_file_store();
    let a = SecretRef::new("integration/a").unwrap();
    let b = SecretRef::new("integration/b").unwrap();

    store.set(&a, "val-a").await.unwrap();
    store.set(&b, "val-b").await.unwrap();

    assert_eq!(store.get(&a).await.unwrap().as_deref(), Some("val-a"));
    assert_eq!(store.get(&b).await.unwrap().as_deref(), Some("val-b"));

    store.delete(&a).await.unwrap();
    assert!(store.get(&a).await.unwrap().is_none());
    assert_eq!(store.get(&b).await.unwrap().as_deref(), Some("val-b"));
}

#[tokio::test]
async fn file_store_exists_correct_integration() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/exists-check").unwrap();

    assert!(!store.exists(&key).await.unwrap());

    store.set(&key, "yep").await.unwrap();
    assert!(store.exists(&key).await.unwrap());

    store.delete(&key).await.unwrap();
    assert!(!store.exists(&key).await.unwrap());
}

#[tokio::test]
async fn file_store_empty_string_value_is_stored() {
    let (store, _dir) = temp_file_store();
    let key = SecretRef::new("integration/empty").unwrap();

    store.set(&key, "").await.unwrap();
    // An empty string is a valid stored value; it is NOT the same as "not found".
    assert_eq!(store.get(&key).await.unwrap().as_deref(), Some(""));
}

// ── KeyringFirstSecretStore integration tests ───────────────────────

fn temp_keyring_first_store() -> (KeyringFirstSecretStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = KeyringFirstSecretStore::new(
        "com.meetily.secrets.test.integration",
        dir.path().join("secrets.yml"),
    );
    (store, dir)
}

#[tokio::test]
async fn keyring_first_set_and_get_via_file_fallback() {
    // Keyring may not be available in CI — the file fallback should handle it.
    let (store, _dir) = temp_keyring_first_store();
    let key = SecretRef::new("integration/kfs/k1").unwrap();

    store.set(&key, "kfs-secret").await.unwrap();
    let got = store.get(&key).await.unwrap();

    assert_eq!(got.as_deref(), Some("kfs-secret"));
}

#[tokio::test]
async fn keyring_first_get_missing_returns_none() {
    let (store, _dir) = temp_keyring_first_store();
    let key = SecretRef::new("integration/kfs/missing").unwrap();

    let got = store.get(&key).await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn keyring_first_delete_removes_from_file() {
    let (store, _dir) = temp_keyring_first_store();
    let key = SecretRef::new("integration/kfs/to-delete").unwrap();

    store.set(&key, "delete-me").await.unwrap();
    store.delete(&key).await.unwrap();

    assert!(!store.exists(&key).await.unwrap());
}

#[tokio::test]
async fn keyring_first_exists_correctly() {
    let (store, _dir) = temp_keyring_first_store();
    let key = SecretRef::new("integration/kfs/exists-check").unwrap();

    assert!(!store.exists(&key).await.unwrap());

    store.set(&key, "val").await.unwrap();
    assert!(store.exists(&key).await.unwrap());

    store.delete(&key).await.unwrap();
    assert!(!store.exists(&key).await.unwrap());
}

#[tokio::test]
async fn keyring_first_default_store_constructor_works() {
    let result = KeyringFirstSecretStore::default_store();
    assert!(result.is_ok());
}

// ── SecretRef validation integration tests ──────────────────────────

#[test]
fn secret_ref_valid_namespace_patterns() {
    let valid = [
        "provider/summary/openai/api_key",
        "provider/transcript/whisper/api_key",
        "provider/summary/custom-openai/api_key",
        "knowledge_graph/lightrag/api_key",
        "a",
        "a/b",
        "a/b/c",
        "alpha_123/bravo-456",
    ];

    for ns in &valid {
        let r = SecretRef::new(*ns);
        assert!(r.is_ok(), "expected '{}' to be valid", ns);
    }
}

#[test]
fn secret_ref_invalid_namespace_patterns() {
    let invalid = [
        "",
        "provider summary",
        "provider@summary",
        "key=value",
        "provider\nsummary",
        "provider\tkey",
    ];

    for ns in &invalid {
        let r = SecretRef::new(*ns);
        assert!(r.is_err(), "expected '{}' to be invalid", ns);
    }
}

#[test]
fn secret_ref_display_integration() {
    let r = SecretRef::new("integration/display").unwrap();
    assert_eq!(format!("{}", r), "integration/display");
}

#[test]
fn secret_ref_serde_roundtrip_integration() {
    let r = SecretRef::new("integration/serde/test").unwrap();
    let json = serde_json::to_string(&r).unwrap();
    let restored: SecretRef = serde_json::from_str(&json).unwrap();
    assert_eq!(r, restored);
    assert_eq!(restored.as_str(), "integration/serde/test");
}
