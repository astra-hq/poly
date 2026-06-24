use log::info;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::io::{BufRead, Read};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::knowledge_graph::config::{
    self, EmbeddingConfig, KnowledgeGraphProfile, KnowledgeGraphSelection,
    KnowledgeGraphSettings, ProfileKind,
};
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::state::AppState;

// ── Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DependencyInfo {
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupDependencyResult {
    pub docker: DependencyInfo,
    pub ollama: DependencyInfo,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Load knowledge-graph settings from the `settings` table.
///
/// Returns defaults (with the built-in default profile) when no settings
/// have been persisted yet, so commands work out of the box.
pub(crate) async fn load_kg_settings(
    pool: &SqlitePool,
) -> Result<KnowledgeGraphSettings, String> {
    let row =
        sqlx::query("SELECT knowledge_graph_settings FROM settings LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                format!("Failed to query knowledge graph settings: {}", e)
            })?;

    match row {
        Some(row) => {
            let json: Option<String> =
                row.try_get("knowledge_graph_settings").ok().flatten();
            match json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    format!(
                        "Failed to parse knowledge graph settings: {}",
                        e
                    )
                }),
                None => Ok(KnowledgeGraphSettings::default()),
            }
        }
        None => Ok(KnowledgeGraphSettings::default()),
    }
}

/// Persist knowledge-graph settings as JSON into the `settings` table.
///
/// Uses UPSERT (`ON CONFLICT(id) DO UPDATE`) so the column is updated
/// whether or not a settings row already exists.
pub(crate) async fn save_kg_settings(
    pool: &SqlitePool,
    settings: &KnowledgeGraphSettings,
) -> Result<(), String> {
    // Validate before persisting
    config::validate(settings).map_err(|e| e.to_string())?;

    let json =
        serde_json::to_string(settings).map_err(|e| {
            format!("Failed to serialize knowledge graph settings: {}", e)
        })?;

    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel, knowledge_graph_settings)
        VALUES ('1', 'openai', 'gpt-4o-2024-11-20', 'large-v3', $1)
        ON CONFLICT(id) DO UPDATE SET
            knowledge_graph_settings = excluded.knowledge_graph_settings
        "#,
    )
    .bind(&json)
    .execute(pool)
    .await
    .map_err(|e| {
        format!("Failed to save knowledge graph settings: {}", e)
    })?;

    Ok(())
}

// ── Commands ────────────────────────────────────────────────────────────

/// Retrieve the current knowledge graph settings.
///
/// Returns defaults when nothing has been persisted yet.
#[tauri::command]
pub async fn api_get_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<KnowledgeGraphSettings, String> {
    info!("api_get_knowledge_graph_settings called");
    let pool = state.db_manager.pool();
    load_kg_settings(pool).await
}

/// Save (validate and persist) knowledge graph settings.
///
/// Rejects empty profiles, duplicate IDs, invalid URLs, etc.
#[tauri::command]
pub async fn api_save_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: KnowledgeGraphSettings,
) -> Result<KnowledgeGraphSettings, String> {
    info!(
        "api_save_knowledge_graph_settings: {} profiles",
        settings.profiles.len()
    );
    let pool = state.db_manager.pool();
    save_kg_settings(pool, &settings).await?;
    // Re-read so the caller gets the canonical stored copy.
    load_kg_settings(pool).await
}

/// Health-check a specific profile by calling its LightRAG endpoint.
///
/// Returns `{ healthy: bool, version?: string }`.
#[tauri::command]
pub async fn api_test_knowledge_graph_profile<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<serde_json::Value, String> {
    info!(
        "api_test_knowledge_graph_profile: profile={}",
        profile_id
    );

    // ── Validate profile_id ──────────────────────────────────────
    if profile_id.is_empty() || profile_id.eq_ignore_ascii_case("none") {
        return Err(format!("Invalid profile_id: '{}'", profile_id));
    }

    let pool = state.db_manager.pool();
    let settings = load_kg_settings(pool).await?;

    let profile = settings
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| {
            format!(
                "Profile '{}' not found in knowledge graph settings",
                profile_id
            )
        })?;

    let provider = LightRagProvider::new(
        &profile.lightrag_url,
        profile.api_key.clone(),
    )
    .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    match provider.health().await {
        Ok(health) => {
            let mut result =
                serde_json::json!({ "healthy": health.healthy });
            if let Some(version) = &health.version {
                result["version"] = serde_json::Value::String(version.clone());
            }
            Ok(result)
        }
        Err(e) => Err(format!(
            "Health check failed for profile '{}': {}",
            profile_id, e
        )),
    }
}

// ── Event helpers ──────────────────────────────────────────────────────

/// Emit a `setup-progress` event to the frontend during the auto-provisioning
/// flow so the UI can display a scrolling log view.
fn emit_setup_progress<R: Runtime>(
    app: &AppHandle<R>,
    message: &str,
    level: &str,
    stage: &str,
) {
    let _ = app.emit(
        "setup-progress",
        serde_json::json!({
            "message": message,
            "level": level,
            "stage": stage,
        }),
    );
}

// ── Commands ────────────────────────────────────────────────────────────

/// Check whether Docker and Ollama are installed and report their versions.
/// This is Phase 1 of the "Set it up for me" flow — the frontend uses the
/// result to decide whether to offer the local (Ollama) path.
#[tauri::command]
pub async fn api_setup_kg_check_deps() -> Result<SetupDependencyResult, String> {
    info!("api_setup_kg_check_deps called");

    let docker = check_docker();
    let ollama = check_ollama();

    Ok(SetupDependencyResult { docker, ollama })
}

/// Check whether Docker is installed (docker --version).
fn check_docker() -> DependencyInfo {
    match std::process::Command::new("docker")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            DependencyInfo {
                installed: true,
                version: Some(version),
            }
        }
        _ => DependencyInfo {
            installed: false,
            version: None,
        },
    }
}

/// Check whether Ollama is installed (ollama --version).
fn check_ollama() -> DependencyInfo {
    match std::process::Command::new("ollama")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            DependencyInfo {
                installed: true,
                version: Some(version),
            }
        }
        _ => DependencyInfo {
            installed: false,
            version: None,
        },
    }
}

/// Pull an embedding model via Ollama, emitting progress events.
/// This is Phase 2 of the "Set it up for me" flow.
#[tauri::command]
pub async fn api_setup_kg_pull_model<R: Runtime>(
    app: AppHandle<R>,
    model: String,
) -> Result<String, String> {
    info!("api_setup_kg_pull_model called for model: {}", model);

    emit_setup_progress(
        &app,
        &format!("Pulling embedding model '{}' via Ollama...", model),
        "info",
        "pull-model",
    );

    let mut child = std::process::Command::new("ollama")
        .args(["pull", &model])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            let msg = format!("Failed to run ollama pull: {}. Is Ollama installed?", e);
            emit_setup_progress(&app, &msg, "error", "error");
            msg
        })?;

    // ollama pull uses \r-based progress bars, not just \n-terminated lines.
    // Reading raw bytes and splitting on both \r and \n gives us every
    // incremental progress update instead of only the final output.
    if let Some(stdout) = child.stdout.take() {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line_buf = Vec::new();
        loop {
            match reader.read_until(b'\n', &mut line_buf) {
                Ok(0) => break,
                Ok(_) => {
                    while let Some(&last) = line_buf.last() {
                        if last == b'\r' || last == b'\n' {
                            line_buf.pop();
                        } else {
                            break;
                        }
                    }
                    if !line_buf.is_empty() {
                        for chunk in line_buf.split(|&b| b == b'\r') {
                            let trimmed = String::from_utf8_lossy(chunk)
                                .trim()
                                .to_string();
                            if !trimmed.is_empty() {
                                emit_setup_progress(
                                    &app, &trimmed, "info", "pull-model",
                                );
                            }
                        }
                    }
                    line_buf.clear();
                }
                Err(e) => {
                    info!("Error reading ollama pull stdout: {}", e);
                    break;
                }
            }
        }
    }

    let status = child.wait().map_err(|e| {
        let msg = format!("Failed to wait for ollama pull: {}", e);
        emit_setup_progress(&app, &msg, "error", "error");
        msg
    })?;

    if !status.success() {
        let stderr = child.stderr.take().map(|s| {
            let mut buf = String::new();
            std::io::BufReader::new(s).read_to_string(&mut buf).ok();
            buf
        }).unwrap_or_default();
        let msg = if stderr.trim().is_empty() {
            format!("Ollama pull failed (exit code: {:?})", status.code())
        } else {
            format!("Ollama pull failed: {}", stderr.trim())
        };
        emit_setup_progress(&app, &msg, "error", "error");
        return Err(msg);
    }

    emit_setup_progress(
        &app,
        &format!("Model '{}' pulled successfully", model),
        "success",
        "pull-model",
    );

    Ok(format!("Model '{}' is ready", model))
}

/// Auto-provision a local knowledge graph stack via docker compose.
///
/// Creates or updates `kg.env` with secure credentials, writes LightRAG
/// Ollama config into `.env`, runs `docker compose -f docker-compose.kg.yml up -d`,
/// waits for LightRAG to become healthy, and auto-creates a local profile.
///
/// Emits `setup-progress` events during each phase.
#[tauri::command]
pub async fn api_setup_local_knowledge_graph<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    info!("api_setup_local_knowledge_graph called");

    // ── Check Docker availability ──────────────────────────────────
    emit_setup_progress(
        &app,
        "Checking Docker availability...",
        "info",
        "docker-check",
    );

    let docker_check = std::process::Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|_| {
            let msg = "Docker is not installed or not in PATH. Please install Docker Desktop (https://docker.com) and try again.";
            emit_setup_progress(&app, msg, "error", "error");
            msg.to_string()
        })?;

    if !docker_check.status.success() {
        let msg = "Docker is not running or accessible.";
        emit_setup_progress(&app, msg, "error", "error");
        return Err(msg.to_string());
    }

    let docker_version =
        String::from_utf8_lossy(&docker_check.stdout).trim().to_string();
    info!("Docker available: {}", docker_version);
    emit_setup_progress(
        &app,
        &format!("Docker available: {}", docker_version),
        "success",
        "docker-check",
    );

    // ── Resolve docker-compose file path ───────────────────────────
    // Try the resource directory first (production), then cwd (dev).
    let compose_path = {
        let candidates = vec![
            // Production: bundled resource via resource resolver
            app.path().resource_dir().ok().map(|d| {
                d.join("docker-compose.kg.yml")
            }),
            // Dev: current working directory (project root)
            std::env::current_dir().ok().map(|d| {
                d.join("docker-compose.kg.yml")
            }),
            // Dev: one level up (frontend/src-tauri -> project root)
            std::env::current_dir().ok().map(|d| {
                d.join("..").join("..").join("docker-compose.kg.yml")
            }),
            // Dev: two levels up (frontend/src-tauri -> project root)
            std::env::current_dir().ok().map(|d| {
                d.join("..").join("docker-compose.kg.yml")
            }),
        ];

        candidates.into_iter().flatten().find(|p| p.exists())
    };

    let compose_path = compose_path.ok_or_else(|| {
        let msg = "Could not find docker-compose.kg.yml. Ensure it is present in the project root.";
        emit_setup_progress(&app, msg, "error", "error");
        msg.to_string()
    })?;

    info!("Using compose file: {:?}", compose_path);
    emit_setup_progress(&app, "Found docker-compose.kg.yml", "success", "compose-file");

    // ── Ensure kg.env exists ───────────────────────────────────────
    let env_path = compose_path.parent().unwrap().join("kg.env");
    if !env_path.exists() {
        emit_setup_progress(
            &app,
            "Creating kg.env with secure credentials...",
            "info",
            "env-file",
        );
        use std::time::{SystemTime, UNIX_EPOCH};
        let random_suffix: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let neo4j_password = format!("meetily-{:x}", random_suffix);
        let env_content = format!(
            "# Auto-generated by Meetily knowledge-graph setup\n\
             NEO4J_AUTH=neo4j/{}\n\
             RUSTFS_ACCESS_KEY=meetily\n\
             RUSTFS_SECRET_KEY=meetily-secret-{:x}\n\
             LIGHTRAG_API_KEY=meetily-lrag-{:x}\n\
             \n\
             # LightRAG / embedding model reference\n\
             LIGHTRAG_EMBEDDING_MODEL=bge-m3\n\
             LIGHTRAG_EMBEDDING_MODEL_NAME=BAAI/bge-m3\n",
            neo4j_password,
            random_suffix.wrapping_add(1),
            random_suffix.wrapping_add(2),
        );

        std::fs::write(&env_path, env_content).map_err(|e| {
            let msg = format!("Failed to create kg.env: {}", e);
            emit_setup_progress(&app, &msg, "error", "error");
            msg
        })?;
        info!("Created kg.env at {:?}", env_path);
        emit_setup_progress(
            &app,
            "kg.env created with secure credentials",
            "success",
            "env-file",
        );
    }

    // ── Read LIGHTRAG_API_KEY from kg.env (must exist by now) ──────
    // This key is needed both in the container .env (for LightRAG auth) and
    // in the profile (so the frontend can authenticate requests).
    let kg_env_raw =
        std::fs::read_to_string(&env_path).unwrap_or_default();
    let lightrag_api_key: String = kg_env_raw
        .lines()
        .find(|l| l.starts_with("LIGHTRAG_API_KEY="))
        .and_then(|l| l.splitn(2, '=').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "meetily-lrag-default".to_string());

    // ── Write LightRAG Ollama config into .env ─────────────────────
    emit_setup_progress(
        &app,
        "Configuring LightRAG to use Ollama for embeddings...",
        "info",
        "env-file",
    );

    let dot_env_path = compose_path.parent().unwrap().join(".env");
    let dot_env_content = format!(
        "\
# LightRAG — auto-generated by Meetily setup\n\
LIGHTRAG_API_KEY={}\n\
LLM_BINDING=ollama\n\
LLM_BINDING_HOST=http://host.docker.internal:11434\n\
LLM_MODEL=qwen2.5:1.5b\n\
\n\
EMBEDDING_BINDING=ollama\n\
EMBEDDING_BINDING_HOST=http://host.docker.internal:11434\n\
EMBEDDING_MODEL=bge-m3:latest\n\
EMBEDDING_DIM=1024\n",
        lightrag_api_key
    );
    std::fs::write(&dot_env_path, dot_env_content).map_err(|e| {
        let msg = format!("Failed to write .env: {}", e);
        emit_setup_progress(&app, &msg, "error", "error");
        msg
    })?;
    info!("Wrote LightRAG Ollama config to {:?}", dot_env_path);
    emit_setup_progress(
        &app,
        "LightRAG configured to use Ollama",
        "success",
        "env-file",
    );

    // ── Run docker compose up ──────────────────────────────────────
    emit_setup_progress(
        &app,
        "Starting Docker containers (this may take a minute)...",
        "info",
        "compose-up",
    );

    // Bring up all services, force-recreate lightrag so it always
    // picks up the freshly-written .env config (LIGHTRAG_API_KEY, etc.)
    let output = std::process::Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_path.to_string_lossy(),
            "--env-file",
            &env_path.to_string_lossy(),
            "up",
            "-d",
            "--force-recreate",
            "lightrag",
        ])
        .output()
        .map_err(|e| {
            let msg = format!("Failed to run docker compose: {}", e);
            emit_setup_progress(&app, &msg, "error", "error");
            msg
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        emit_setup_progress(
            &app,
            &format!("Docker compose failed: {}", stderr),
            "error",
            "error",
        );
        return Err(format!("Docker compose failed:\n{}", stderr));
    }

    let compose_out = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let compose_err = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let summary = if !compose_out.is_empty() {
        compose_out
    } else {
        compose_err
    };
    if !summary.is_empty() {
        emit_setup_progress(&app, &summary, "info", "compose-up");
    }
    info!("Docker compose stack started");

    // ── Poll LightRAG health ──────────────────────────────────────
    emit_setup_progress(
        &app,
        "Containers started. Waiting for LightRAG to become healthy...",
        "info",
        "waiting-health",
    );

    let env_content = std::fs::read_to_string(&env_path)
        .map_err(|e| format!("Failed to read kg.env: {}", e))?;

    let api_key: Option<String> = env_content
        .lines()
        .find(|l| l.starts_with("LIGHTRAG_API_KEY="))
        .and_then(|l| l.splitn(2, '=').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let health_client = reqwest::Client::new();
    let health_url = "http://localhost:9621/health";
    let max_attempts: u32 = 40;

    for attempt in 1..=max_attempts {
        let mut req = health_client
            .get(health_url)
            .timeout(Duration::from_secs(5));
        if let Some(ref key) = api_key {
            req = req.header("X-API-Key", key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("LightRAG health check passed on attempt {}", attempt);
                emit_setup_progress(
                    &app,
                    "LightRAG is healthy and ready!",
                    "success",
                    "waiting-health",
                );
                break;
            }
            Ok(resp) => {
                info!(
                    "LightRAG health check attempt {}: status={}",
                    attempt,
                    resp.status()
                );
            }
            Err(e) => {
                info!("LightRAG health check attempt {}: {}", attempt, e);
            }
        }

        if attempt < max_attempts {
            // Emit a heartbeat every 10 attempts so the UI doesn't go silent
            if attempt % 10 == 0 {
                emit_setup_progress(
                    &app,
                    &format!(
                        "Still waiting for LightRAG... check {}/{}",
                        attempt, max_attempts
                    ),
                    "info",
                    "waiting-health",
                );
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        } else {
            let msg = "LightRAG did not become healthy within the timeout (120s). \
                       Check 'docker compose -f docker-compose.kg.yml logs' for details.";
            emit_setup_progress(&app, msg, "error", "error");
            return Err(msg.to_string());
        }
    }

    // ── Create and persist a local profile ────────────────────────
    emit_setup_progress(
        &app,
        "Creating local knowledge graph profile...",
        "info",
        "creating-profile",
    );

    let embedding_model: String = env_content
        .lines()
        .find(|l| l.starts_with("LIGHTRAG_EMBEDDING_MODEL_NAME="))
        .and_then(|l| l.splitn(2, '=').nth(1))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "BAAI/bge-m3".to_string());

    let profile = KnowledgeGraphProfile {
        id: "local-lightrag".to_string(),
        name: "Local LightRAG".to_string(),
        kind: ProfileKind::Local,
        embedding: EmbeddingConfig {
            provider: "ollama".to_string(),
            model: embedding_model.clone(),
            dimensions: 1024,
        },
        lightrag_url: "http://localhost:9621".to_string(),
        api_key: api_key.clone(),
        notes: Some("Auto-provisioned by 'Set it up for me'".to_string()),
    };

    let pool = state.db_manager.pool();
    let mut settings = load_kg_settings(pool).await?;
    settings.profiles.retain(|p| p.id != "local-lightrag");
    settings.profiles.push(profile.clone());
    settings.active_profile =
        KnowledgeGraphSelection::Profile("local-lightrag".to_string());
    save_kg_settings(pool, &settings).await?;

    info!("Local LightRAG profile created and set as active");
    emit_setup_progress(
        &app,
        "Profile 'Local LightRAG' created and set as default",
        "success",
        "creating-profile",
    );
    emit_setup_progress(
        &app,
        "Setup complete! Your local knowledge graph is ready to use.",
        "success",
        "done",
    );

    Ok(format!(
        "Knowledge graph stack is running.\n\
         LightRAG: http://localhost:9621\n\
         Profile: {} (active)\n\
         API Key: {}",
        profile.name,
        api_key.as_deref().unwrap_or("(none)"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::config::{
        KnowledgeGraphProfile, KnowledgeGraphSelection,
        ProfileKind,
    };
    use httpmock::MockServer;
    use serde_json::json;
    use sqlx::SqlitePool;

    /// Create an in-memory SQLite database with the app schema applied.
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    // ── load_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn load_returns_defaults_when_no_settings_row() {
        let pool = test_pool().await;

        // No row inserted — should return defaults.
        let settings = load_kg_settings(&pool).await.unwrap();
        assert_eq!(settings.profiles.len(), 0);
        assert_eq!(settings.active_profile, KnowledgeGraphSelection::None);
    }

    #[tokio::test]
    async fn load_returns_defaults_when_column_is_null() {
        let pool = test_pool().await;

        // Insert row but leave knowledge_graph_settings NULL.
        sqlx::query(
            "INSERT INTO settings (id, provider, model, whisperModel) VALUES ('1', 'openai', 'gpt-4o', 'large-v3')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let settings = load_kg_settings(&pool).await.unwrap();
        assert_eq!(settings.profiles.len(), 0);
        assert_eq!(settings.active_profile, KnowledgeGraphSelection::None);
    }

    #[tokio::test]
    async fn load_returns_persisted_settings() {
        let pool = test_pool().await;

        let original = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "prod-1".into(),
                name: "production".into(),
                kind: ProfileKind::Remote,
                lightrag_url: "http://kg.example.com".into(),
                api_key: Some("key-123".into()),
                notes: Some("main cluster".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("prod-1".into()),
        };
        save_kg_settings(&pool, &original).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded, original);
    }

    // ── save_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn save_accepts_empty_profiles() {
        let pool = test_pool().await;

        let empty = KnowledgeGraphSettings {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        };
        assert!(save_kg_settings(&pool, &empty).await.is_ok());
        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded.profiles.len(), 0);
    }

    #[tokio::test]
    async fn save_rejects_empty_profile_name() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("Profile name cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_empty_lightrag_url() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "x".into(),
                lightrag_url: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("LightRAG URL cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_missing_active_profile() {
        let pool = test_pool().await;

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "a".into(),
                name: "Alpha".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("b".into()),
        };
        let err = save_kg_settings(&pool, &invalid).await.unwrap_err();
        assert!(err.contains("Active profile 'b' not found"));
    }

    #[tokio::test]
    async fn save_accepts_valid_url_with_dummy_api_key() {
        let pool = test_pool().await;

        let valid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "local-1".into(),
                name: "local dev".into(),
                kind: ProfileKind::Local,
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("dummy-key".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("local-1".into()),
        };
        assert!(save_kg_settings(&pool, &valid).await.is_ok());
        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded, valid);
    }

    // ── Profile CRUD roundtrip ──────────────────────────────────────

    #[tokio::test]
    async fn profile_add_update_delete_roundtrip() {
        let pool = test_pool().await;

        let initial = load_kg_settings(&pool).await.unwrap();
        assert_eq!(initial.profiles.len(), 0);

        let mut settings = initial.clone();
        settings.profiles.push(KnowledgeGraphProfile {
            id: "remote-1".into(),
            name: "cloud kg".into(),
            kind: ProfileKind::Remote,
            lightrag_url: "https://kg.example.com".into(),
            api_key: Some("remote-key".into()),
            notes: Some("team server".into()),
            ..Default::default()
        });
        save_kg_settings(&pool, &settings).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].id, "remote-1");

        let mut updated = loaded.clone();
        if let Some(p) = updated.profiles.iter_mut().find(|p| p.id == "remote-1") {
            p.name = "cloud kg v2".into();
            p.lightrag_url = "https://kg2.example.com".into();
        }
        save_kg_settings(&pool, &updated).await.unwrap();

        let loaded2 = load_kg_settings(&pool).await.unwrap();
        let p = loaded2.profiles.iter().find(|p| p.id == "remote-1").unwrap();
        assert_eq!(p.name, "cloud kg v2");
        assert_eq!(p.lightrag_url, "https://kg2.example.com");

        let mut pruned = loaded2.clone();
        pruned.profiles.retain(|p| p.id != "remote-1");
        save_kg_settings(&pool, &pruned).await.unwrap();

        let loaded3 = load_kg_settings(&pool).await.unwrap();
        assert_eq!(loaded3.profiles.len(), 0);
    }

    // ── Health check tests with mock HTTP ───────────────────────────

    #[tokio::test]
    async fn health_test_healthy_with_mock_server() {
        let pool = test_pool().await;
        let server = MockServer::start();

        // Persist a profile pointing at the mock server
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "health-1".into(),
                name: "mock".into(),
                kind: ProfileKind::Local,
                lightrag_url: server.base_url(),
                api_key: Some("secret".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("health-1".into()),
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        // Mock the /health endpoint
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/health")
                .header("X-API-Key", "secret");
            then.status(200)
                .json_body(json!({"healthy": true, "version": "2.0.0"}));
        });

        // Resolve and call health through the provider
        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "health-1")
            .unwrap();
        let provider =
            LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
                .unwrap();
        let health = provider.health().await.unwrap();

        mock.assert();
        assert!(health.healthy);
        assert_eq!(health.version.as_deref(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn health_test_unhealthy_with_mock_server() {
        let pool = test_pool().await;
        let server = MockServer::start();

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "unhealthy-1".into(),
                name: "broken".into(),
                kind: ProfileKind::Remote,
                lightrag_url: server.base_url(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        // Mock endpoint returns 500
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/health");
            then.status(500).body("internal error");
        });

        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "unhealthy-1")
            .unwrap();
        let provider =
            LightRagProvider::new(&profile.lightrag_url, profile.api_key.clone())
                .unwrap();
        let result = provider.health().await;

        mock.assert();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500") || err.contains("internal error"),
            "expected error to mention 500 or 'internal error', got: {err}");
    }

    #[tokio::test]
    async fn health_test_profile_not_found() {
        let pool = test_pool().await;

        // No profile with id "nonexistent"
        let loaded = load_kg_settings(&pool).await.unwrap();
        let found = loaded
            .profiles
            .iter()
            .find(|p| p.id == "nonexistent");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn health_test_connection_refused() {
        let pool = test_pool().await;

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "dead".into(),
                name: "unreachable".into(),
                kind: ProfileKind::Local,
                // Use a port that is almost certainly not bound
                lightrag_url: "http://127.0.0.1:19999".into(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&pool, &settings).await.unwrap();

        let loaded = load_kg_settings(&pool).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "dead")
            .unwrap();
        let result = LightRagProvider::new(
            &profile.lightrag_url,
            profile.api_key.clone(),
        )
        .unwrap()
        .health()
        .await;

        assert!(result.is_err(), "expected connection error");
    }
}
