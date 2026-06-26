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
        "com.poly.secrets.test.integration",
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

// ── Migration-aware KeyringFirstSecretStore integration tests ────────

fn temp_migration_store(
    legacy_file_path: std::path::PathBuf,
) -> (KeyringFirstSecretStore, std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let poly_path = dir.path().join("poly_secrets.yml");
    let store = KeyringFirstSecretStore::with_legacy(
        "com.poly.secrets.test.integration",
        poly_path.clone(),
        "com.meetily.secrets.test.legacy",
        legacy_file_path,
    );
    (store, poly_path, dir)
}

#[tokio::test]
async fn migration_store_reads_from_legacy_file_fallback() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let legacy_store = FileSecretStore::new(legacy_path.clone());
    let key = SecretRef::new("integration/migration/fallback-key").unwrap();
    legacy_store.set(&key, "legacy-integration-value").await.unwrap();

    let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

    let val = store.get(&key).await.unwrap();
    assert_eq!(val.as_deref(), Some("legacy-integration-value"));
}

#[tokio::test]
async fn migration_store_copies_legacy_forward_to_poly_primary() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let legacy_store = FileSecretStore::new(legacy_path.clone());
    let key = SecretRef::new("integration/migration/copy-forward-int").unwrap();
    legacy_store.set(&key, "pre-migration-int-value").await.unwrap();

    let (store, poly_path, _dir) = temp_migration_store(legacy_path.clone());

    // First read triggers copy-forward from legacy to Poly.
    let val = store.get(&key).await.unwrap();
    assert_eq!(val.as_deref(), Some("pre-migration-int-value"));

    // Poly primary file must now have the value.
    let poly_store = FileSecretStore::new(poly_path);
    let poly_val = poly_store.get(&key).await.unwrap();
    assert_eq!(
        poly_val.as_deref(),
        Some("pre-migration-int-value"),
        "value must be copied forward to Poly primary file"
    );
}

#[tokio::test]
async fn migration_store_poly_primary_wins_over_legacy() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let legacy_store = FileSecretStore::new(legacy_path.clone());
    let key = SecretRef::new("integration/migration/primary-wins-int").unwrap();
    legacy_store.set(&key, "legacy-val").await.unwrap();

    let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

    // Write a Poly primary value that differs from legacy.
    store.set(&key, "poly-primary-val").await.unwrap();

    let val = store.get(&key).await.unwrap();
    assert_eq!(val.as_deref(), Some("poly-primary-val"));
}

#[tokio::test]
async fn migration_store_delete_does_not_touch_legacy() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let legacy_store = FileSecretStore::new(legacy_path.clone());
    let key = SecretRef::new("integration/migration/keep-legacy-int").unwrap();
    legacy_store.set(&key, "keep-me-int").await.unwrap();

    let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

    // Delete through migration store.
    store.delete(&key).await.unwrap();

    // Legacy store must still have the value.
    let legacy_val = legacy_store.get(&key).await.unwrap();
    assert_eq!(
        legacy_val.as_deref(),
        Some("keep-me-int"),
        "legacy entry must not be deleted"
    );
}

#[tokio::test]
async fn migration_store_set_writes_only_to_primary_not_legacy() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let legacy_store = FileSecretStore::new(legacy_path.clone());
    let key = SecretRef::new("integration/migration/set-primary-only-int").unwrap();

    let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());

    store.set(&key, "poly-only-int").await.unwrap();

    // Legacy must not have this key.
    let legacy_val = legacy_store.get(&key).await.unwrap();
    assert!(legacy_val.is_none(), "set must not write to legacy store");

    // Primary got the value.
    let val = store.get(&key).await.unwrap();
    assert_eq!(val.as_deref(), Some("poly-only-int"));
}

#[tokio::test]
async fn migration_store_missing_secret_returns_none() {
    let legacy_dir = tempfile::tempdir().unwrap();
    let legacy_path = legacy_dir.path().join("secrets.yml");
    let (store, _poly_path, _dir) = temp_migration_store(legacy_path.clone());
    let key = SecretRef::new("integration/migration/truly-missing-int").unwrap();

    let val = store.get(&key).await.unwrap();
    assert!(val.is_none());
}
