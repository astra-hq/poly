use app_lib::database::manager::DatabaseManager;
use sha2::{Digest, Sha256};

const MODIFIED_LEGACY_MIGRATION_CONTENTS: &[(i64, &str)] = &[
    (
        20250916100000,
        include_str!("../migrations/20250916100000_initial_schema.sql"),
    ),
    (
        20250917000000,
        include_str!("../migrations/20250917000000_add_knowledge_graph_settings.sql"),
    ),
    (
        20250920155811,
        include_str!("../migrations/20250920155811_add_openrouter_api_key.sql"),
    ),
    (
        20251010153942,
        include_str!("../migrations/20251010153942_add_ollama_endpoint.sql"),
    ),
    (
        20251105120000,
        include_str!("../migrations/20251105120000_add_pro_license_custom_openai.sql"),
    ),
    (
        20251229000000,
        include_str!("../migrations/20251229000000_add_gemini_api_key.sql"),
    ),
];

#[tokio::test]
async fn database_manager_rewrites_legacy_migration_checksums_and_preserves_runtime_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("meeting_minutes.sqlite");
    let legacy_path = dir.path().join("missing_legacy.db");
    let db_path = db_path.to_string_lossy().to_string();
    let legacy_path = legacy_path.to_string_lossy().to_string();

    let manager = DatabaseManager::new(&db_path, &legacy_path).await.unwrap();
    sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ($1, $2, $3, $4)")
        .bind("meeting-1")
        .bind("Checksum Regression")
        .bind("2026-06-25T00:00:00Z")
        .bind("2026-06-25T00:00:00Z")
        .execute(manager.pool())
        .await
        .unwrap();

    for (version, content) in MODIFIED_LEGACY_MIGRATION_CONTENTS {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let checksum = hasher.finalize().to_vec();

        sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = $2")
            .bind(&checksum)
            .bind(version)
            .execute(manager.pool())
            .await
            .unwrap();
    }

    manager.pool().close().await;

    let manager = DatabaseManager::new(&db_path, &legacy_path).await.unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM meetings WHERE id = $1")
        .bind("meeting-1")
        .fetch_one(manager.pool())
        .await
        .unwrap();
    assert_eq!(title, "Checksum Regression");
}
