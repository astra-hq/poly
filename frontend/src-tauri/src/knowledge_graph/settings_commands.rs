use log::info;
use rand::Rng;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::knowledge_graph::config::{
    self, EmbeddingConfig, KnowledgeGraphProfile, KnowledgeGraphSelection, KnowledgeGraphSettings,
    ProfileKind,
};
use crate::knowledge_graph::lightrag::LightRagProvider;
use crate::knowledge_graph::provider::KnowledgeGraphProvider;
use crate::poly_config::config::{
    KnowledgeGraphProfileWithoutSecrets, KnowledgeGraphSettingsWithoutSecrets,
};
use crate::poly_config::repository::ConfigRepository;
use crate::process_path;
use crate::providers::ProviderType;
use crate::secrets::keyring_first_store::KeyringFirstSecretStore;
use crate::secrets::refs::knowledge_graph_profile_key;
use crate::secrets::status::mask_api_key;
use crate::secrets::store::SecretStore;
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
    pub platform: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Load knowledge-graph settings from YAML config, enriched with secret status.
///
/// For each profile in the YAML config, checks the SecretStore for an API key
/// and populates `has_secret` / `api_key_masked_hint`.  Raw `api_key` is
/// always `None` in the returned value.
///
/// Returns defaults when no YAML config exists yet.
pub(crate) async fn load_kg_settings(
    config_repo: &ConfigRepository,
    store: &dyn SecretStore,
) -> Result<KnowledgeGraphSettings, String> {
    let config = config_repo
        .load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    let mut profiles_with_secrets = Vec::new();
    for p in config.knowledge_graph.profiles {
        let secret_ref = knowledge_graph_profile_key(&p.id);
        let (has_secret, masked_hint) = match store.get(&secret_ref).await {
            Ok(Some(key)) if !key.is_empty() => (true, Some(mask_api_key(&key))),
            _ => (false, None),
        };
        profiles_with_secrets.push(KnowledgeGraphProfile {
            id: p.id,
            name: p.name,
            kind: p.kind,
            embedding: p.embedding,
            lightrag_url: p.lightrag_url,
            api_key: None,
            notes: p.notes,
            has_secret,
            api_key_masked_hint: masked_hint,
            llm_model: p.llm_model,
            llm_provider_id: p.llm_provider_id,
        });
    }

    Ok(KnowledgeGraphSettings {
        profiles: profiles_with_secrets,
        active_profile: config.knowledge_graph.active_profile,
    })
}

/// Persist knowledge-graph settings to YAML via [`ConfigRepository::save_atomic`],
/// saving API keys separately to the SecretStore.
///
/// Secrets for removed profiles are deleted from the SecretStore automatically.
pub(crate) async fn save_kg_settings(
    config_repo: &ConfigRepository,
    store: &dyn SecretStore,
    settings: &KnowledgeGraphSettings,
) -> Result<(), String> {
    config::validate(settings).map_err(|e| e.to_string())?;

    let mut config = config_repo
        .load()
        .map_err(|e| format!("Failed to load config: {}", e))?;

    let old_ids: HashSet<&str> = config
        .knowledge_graph
        .profiles
        .iter()
        .map(|p| p.id.as_str())
        .collect();

    let incoming_ids: HashSet<&str> = settings.profiles.iter().map(|p| p.id.as_str()).collect();

    for p in &settings.profiles {
        let secret_ref = knowledge_graph_profile_key(&p.id);
        match &p.api_key {
            Some(key) if !key.is_empty() => {
                store
                    .set(&secret_ref, key)
                    .await
                    .map_err(|e| format!("Failed to store secret for profile '{}': {}", p.id, e))?;
            }
            Some(_) => {
                store.delete(&secret_ref).await.map_err(|e| {
                    format!("Failed to delete secret for profile '{}': {}", p.id, e)
                })?;
            }
            None => { /* No key sent — leave existing secret untouched */ }
        }
    }

    for removed_id in old_ids.difference(&incoming_ids) {
        let secret_ref = knowledge_graph_profile_key(removed_id);
        let _ = store.delete(&secret_ref).await;
    }

    let without_secrets: Vec<KnowledgeGraphProfileWithoutSecrets> = settings
        .profiles
        .iter()
        .cloned()
        .map(KnowledgeGraphProfileWithoutSecrets::from)
        .collect();

    config.knowledge_graph = KnowledgeGraphSettingsWithoutSecrets {
        profiles: without_secrets,
        active_profile: settings.active_profile.clone(),
    };

    config_repo
        .save_atomic(&config)
        .map_err(|e| format!("Failed to save config: {}", e))
}

// ── Commands ────────────────────────────────────────────────────────────

/// Retrieve the current knowledge graph settings from YAML config.
///
/// Returns defaults when nothing has been persisted yet.
#[tauri::command]
pub async fn api_get_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
) -> Result<KnowledgeGraphSettings, String> {
    info!("api_get_knowledge_graph_settings called");
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    load_kg_settings(&config_repo, &store).await
}

/// Save (validate and persist) knowledge graph settings to YAML config.
///
/// Rejects empty profiles, duplicate IDs, invalid URLs, etc.
#[tauri::command]
pub async fn api_save_knowledge_graph_settings<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    settings: KnowledgeGraphSettings,
) -> Result<KnowledgeGraphSettings, String> {
    info!(
        "api_save_knowledge_graph_settings: {} profiles",
        settings.profiles.len()
    );
    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    save_kg_settings(&config_repo, &store, &settings).await?;
    load_kg_settings(&config_repo, &store).await
}

/// Health-check a specific profile by calling its LightRAG endpoint.
///
/// Returns `{ healthy: bool, version?: string }`.
#[tauri::command]
pub async fn api_test_knowledge_graph_profile<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<serde_json::Value, String> {
    info!("api_test_knowledge_graph_profile: profile={}", profile_id);

    // ── Validate profile_id ──────────────────────────────────────
    if profile_id.is_empty() || profile_id.eq_ignore_ascii_case("none") {
        return Err(format!("Invalid profile_id: '{}'", profile_id));
    }

    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

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

    // Fetch the actual API key from the SecretStore (never returned in settings).
    let secret_ref = knowledge_graph_profile_key(&profile_id);
    let actual_api_key = store
        .get(&secret_ref)
        .await
        .map_err(|e| format!("Failed to read API key for profile '{}': {}", profile_id, e))?
        .filter(|k| !k.is_empty());

    let provider = LightRagProvider::new(&profile.lightrag_url, actual_api_key)
        .map_err(|e| format!("Failed to create LightRag provider: {}", e))?;

    match provider.health().await {
        Ok(health) => {
            let mut result = serde_json::json!({ "healthy": health.healthy });
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
fn emit_setup_progress<R: Runtime>(app: &AppHandle<R>, message: &str, level: &str, stage: &str) {
    let _ = app.emit(
        "setup-progress",
        serde_json::json!({
            "message": message,
            "level": level,
            "stage": stage,
        }),
    );
}

fn poly_docker_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory for Poly docker setup".to_string())?;
    Ok(home.join(".poly").join("docker"))
}

fn prepare_kg_runtime_dir(compose_source_path: &std::path::Path) -> Result<PathBuf, String> {
    let runtime_dir = poly_docker_dir()?;
    std::fs::create_dir_all(runtime_dir.join("data").join("rag_storage"))
        .map_err(|e| format!("Failed to create LightRAG storage directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("inputs"))
        .map_err(|e| format!("Failed to create LightRAG input directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("prompts"))
        .map_err(|e| format!("Failed to create LightRAG prompt directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("neo4j").join("data"))
        .map_err(|e| format!("Failed to create Neo4j data directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("neo4j").join("logs"))
        .map_err(|e| format!("Failed to create Neo4j log directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("neo4j").join("import"))
        .map_err(|e| format!("Failed to create Neo4j import directory: {}", e))?;
    std::fs::create_dir_all(runtime_dir.join("data").join("neo4j").join("plugins"))
        .map_err(|e| format!("Failed to create Neo4j plugin directory: {}", e))?;

    let compose_runtime_path = runtime_dir.join("docker-compose.kg.yml");
    std::fs::copy(compose_source_path, &compose_runtime_path)
        .map_err(|e| format!("Failed to copy docker-compose.kg.yml: {}", e))?;
    Ok(compose_runtime_path)
}

// ── Commands ────────────────────────────────────────────────────────────

/// Check whether Docker and Ollama are installed and report their versions.
/// This is Phase 1 of the "Set it up for me" flow — the frontend uses the
/// result to decide whether to offer the local (Ollama) path.
#[tauri::command]
pub async fn api_setup_kg_check_deps() -> Result<SetupDependencyResult, String> {
    info!("api_setup_kg_check_deps called");

    let docker = check_docker();
    let platform = std::env::consts::OS.to_string();

    Ok(SetupDependencyResult { docker, platform })
}

/// Check whether Docker is installed (docker --version).
fn check_docker() -> DependencyInfo {
    match process_path::command("docker").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
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

/// Resolve the LLM binding config for a KG profile.
///
/// When `llm_provider_id` is set, looks up the provider in the global
/// providers list and maps its type to the LightRAG binding string
/// (`openai`, `anthropic`, `ollama`, etc.). When no provider is
/// configured, defaults to the local bridge via openai binding.
fn resolve_llm_binding(
    config: &crate::poly_config::config::PolyConfig,
    profile: &KnowledgeGraphProfile,
) -> (String, String) {
    let provider_id = match &profile.llm_provider_id {
        Some(id) if !id.is_empty() => id,
        _ => {
            return (
                "openai".into(),
                "http://host.docker.internal:11337/v1".into(),
            )
        }
    };

    match config.find_provider(provider_id) {
        Some(p) => {
            let binding = match p.provider_type {
                ProviderType::OpenAI
                | ProviderType::Groq
                | ProviderType::OpenRouter
                | ProviderType::Custom
                | ProviderType::Local => "openai",
                ProviderType::Anthropic => "anthropic",
                ProviderType::Ollama => "ollama",
            };
            // For Local provider, use the bridge host URL, not whatever base_url is set
            let host = if matches!(p.provider_type, ProviderType::Local) {
                "http://host.docker.internal:11337/v1".to_string()
            } else {
                p.base_url.clone()
            };
            (binding.to_string(), host)
        }
        None => (
            "openai".into(),
            "http://host.docker.internal:11337/v1".into(),
        ),
    }
}

/// Reads the existing `~/.poly/docker/.env`, updates `LLM_BINDING`,
/// `LLM_BINDING_HOST`, `OPENAI_API_KEY`, `LLM_MODEL`,
/// `KEYWORD_LLM_MODEL`, and `QUERY_LLM_MODEL` to match the profile's
/// configured provider, and writes it back. No-op for non-local profiles
/// or when `.env` does not exist yet.
#[tauri::command]
pub async fn api_update_knowledge_graph_env<R: Runtime>(
    _app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    info!("api_update_knowledge_graph_env: profile_id={}", profile_id);

    let config_repo = ConfigRepository::new();
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let settings = load_kg_settings(&config_repo, &store).await?;

    let profile = settings
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| format!("Profile '{}' not found", profile_id))?;

    if profile.kind != ProfileKind::Local {
        return Ok(());
    }

    let runtime_dir = poly_docker_dir()?;
    let dot_env_path = runtime_dir.join(".env");

    if !dot_env_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&dot_env_path)
        .map_err(|e| format!("Failed to read .env: {}", e))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let model = &profile.llm_model;

    // Resolve binding, host, and API key from provider config
    let full_config = config_repo
        .load()
        .map_err(|e| format!("Failed to load config: {}", e))?;
    let (binding, host) = resolve_llm_binding(&full_config, profile);

    // Look up API key from SecretStore for non-Ollama providers
    let api_key = if binding != "ollama" {
        if let Some(ref pid) = profile.llm_provider_id {
            let secret_ref = crate::secrets::refs::summary_provider_key(pid);
            store
                .get(&secret_ref)
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut changed = false;
    let mut has_api_key_line = false;

    for line in &mut lines {
        if line.starts_with("LLM_BINDING=") {
            let new = format!("LLM_BINDING={}", binding);
            if *line != new {
                *line = new;
                changed = true;
            }
        } else if line.starts_with("LLM_BINDING_HOST=") {
            let new = format!("LLM_BINDING_HOST={}", host);
            if *line != new {
                *line = new;
                changed = true;
            }
        } else if line.starts_with("OPENAI_API_KEY=") || line.starts_with("LLM_BINDING_API_KEY=") {
            has_api_key_line = true;
            let new = if line.starts_with("OPENAI_API_KEY=") {
                format!("OPENAI_API_KEY={}", api_key)
            } else {
                format!("LLM_BINDING_API_KEY={}", api_key)
            };
            if *line != new {
                *line = new;
                changed = true;
            }
        } else if line.starts_with("LLM_MODEL=") {
            *line = format!("LLM_MODEL={}", model);
            changed = true;
        } else if line.starts_with("KEYWORD_LLM_MODEL=") {
            *line = format!("KEYWORD_LLM_MODEL={}", model);
            changed = true;
        } else if line.starts_with("QUERY_LLM_MODEL=") {
            *line = format!("QUERY_LLM_MODEL={}", model);
            changed = true;
        }
    }

    // If OPENAI_API_KEY wasn't found in the file, append it after LLM_BINDING_HOST
    if !has_api_key_line {
        let insert_pos = lines
            .iter()
            .position(|l| l.starts_with("LLM_BINDING_HOST="))
            .map(|i| i + 1)
            .unwrap_or(lines.len());
        lines.insert(insert_pos, format!("LLM_BINDING_API_KEY={}", api_key));
        changed = true;
    }

    if changed {
        std::fs::write(&dot_env_path, lines.join("\n"))
            .map_err(|e| format!("Failed to write .env: {}", e))?;
        info!(
            "Updated .env for profile '{}': binding={}, host={}, model={}",
            profile_id, binding, host, model
        );
    }

    Ok(())
}

/// Pure function: generate the `.env` content for a local KG stack.
///
/// Exposed for testing — no side effects, no secret store, no I/O.
fn generate_kg_env_content(
    neo4j_password: &str,
    lightrag_api_key: &str,
    llm_binding: &str,
    llm_binding_host: &str,
    llm_binding_api_key: &str,
    llm_model: &str,
) -> String {
    format!(
        "# Auto-generated by Poly knowledge-graph setup\n\
         NEO4J_AUTH=neo4j/{}\n\
         LIGHTRAG_API_KEY={}\n\
         \n\
         # LightRAG / embedding model reference\n\
         LIGHTRAG_EMBEDDING_MODEL=bge-m3\n\
         LIGHTRAG_EMBEDDING_MODEL_NAME=BAAI/bge-m3\n\
         \n\
         # LightRAG — LLM config\n\
         LLM_BINDING={}\n\
         LLM_BINDING_HOST={}\n\
         LLM_BINDING_API_KEY={}\n\
         LLM_MODEL={}\n\
         KEYWORD_LLM_MODEL={}\n\
         QUERY_LLM_MODEL={}\n\
         \n\
         EMBEDDING_BINDING=openai\n\
         EMBEDDING_BINDING_HOST=http://host.docker.internal:11337/v1\n\
         EMBEDDING_API_KEY=not-needed\n\
         EMBEDDING_MODEL=bge-m3\n\
         EMBEDDING_DIM=1024\n\
         LIGHTRAG_PARSER=*:native-teP,*:legacy-R\n\
         \n\
         # Entity extraction: JSON mode + custom Poly meeting ontology\n\
         ENTITY_EXTRACTION_USE_JSON=true\n\
         ENTITY_TYPE_PROMPT_FILE=poly_meeting_ontology.yml\n\
         PROMPT_DIR=/app/data/prompts",
        neo4j_password,
        lightrag_api_key,
        llm_binding,
        llm_binding_host,
        llm_binding_api_key,
        llm_model,
        llm_model,
        llm_model
    )
}

/// Auto-provision a local knowledge graph stack via docker compose.
///
/// Generates `.env` with secure credentials and LightRAG Ollama config,
/// runs `docker compose -f docker-compose.kg.yml up -d`, waits for LightRAG
/// to become healthy, and auto-creates a local profile.
///
/// Emits `setup-progress` events during each phase.
#[tauri::command]
pub async fn api_setup_local_knowledge_graph<R: Runtime>(
    app: AppHandle<R>,
    _state: tauri::State<'_, AppState>,
    llm_model: String,
    llm_provider_id: Option<String>,
) -> Result<String, String> {
    info!("api_setup_local_knowledge_graph called");

    // ── Check Docker availability ──────────────────────────────────
    emit_setup_progress(
        &app,
        "Checking Docker availability...",
        "info",
        "docker-check",
    );

    let docker_check = process_path::command("docker")
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

    let docker_version = String::from_utf8_lossy(&docker_check.stdout)
        .trim()
        .to_string();
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
            app.path()
                .resource_dir()
                .ok()
                .map(|d| d.join("docker-compose.kg.yml")),
            // Dev: current working directory (project root)
            std::env::current_dir()
                .ok()
                .map(|d| d.join("docker-compose.kg.yml")),
            // Dev: one level up (frontend/src-tauri -> project root)
            std::env::current_dir()
                .ok()
                .map(|d| d.join("..").join("..").join("docker-compose.kg.yml")),
            // Dev: two levels up (frontend/src-tauri -> project root)
            std::env::current_dir()
                .ok()
                .map(|d| d.join("..").join("docker-compose.kg.yml")),
        ];

        candidates.into_iter().flatten().find(|p| p.exists())
    };

    let compose_source_path = compose_path.ok_or_else(|| {
        let msg = "Could not find docker-compose.kg.yml. Ensure it is present in the project root.";
        emit_setup_progress(&app, msg, "error", "error");
        msg.to_string()
    })?;

    info!("Using compose source file: {:?}", compose_source_path);
    let compose_path = prepare_kg_runtime_dir(&compose_source_path).map_err(|e| {
        emit_setup_progress(&app, &e, "error", "error");
        e
    })?;
    let runtime_dir = compose_path
        .parent()
        .ok_or_else(|| "Failed to resolve Poly docker directory".to_string())?;
    emit_setup_progress(
        &app,
        &format!(
            "Prepared Docker runtime directory at {}",
            runtime_dir.display()
        ),
        "success",
        "compose-file",
    );

    // ── Get or create LightRAG API key in SecretStore ────────────
    // The SecretStore is the canonical source for API keys.  Never
    // read LIGHTRAG_API_KEY from kg.env — that file is a generated
    // runtime artifact.
    let store = KeyringFirstSecretStore::default_store()
        .map_err(|e| format!("Failed to initialize secret store: {}", e))?;
    let secret_ref = knowledge_graph_profile_key("local-lightrag");

    let lightrag_api_key = match store
        .get(&secret_ref)
        .await
        .map_err(|e| format!("Failed to read LightRAG secret: {}", e))?
    {
        Some(existing) if !existing.is_empty() => {
            info!("Using existing LightRAG API key from SecretStore");
            existing
        }
        _ => {
            emit_setup_progress(&app, "Creating new LightRAG API key...", "info", "env-file");
            let bytes: [u8; 32] = rand::thread_rng().gen();
            let new_key = format!(
                "poly-lrag-{}",
                bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            );
            store
                .set(&secret_ref, &new_key)
                .await
                .map_err(|e| format!("Failed to store LightRAG API key: {}", e))?;
            info!("Created new LightRAG API key in SecretStore");
            new_key
        }
    };

    // ── Generate consolidated .env (runtime artifact, not config source) ────
    emit_setup_progress(
        &app,
        "Generating .env with secure credentials and LightRAG config...",
        "info",
        "env-file",
    );

    // ── Resolve LLM binding, host, and API key from provider config ────
    let (llm_binding, llm_binding_host, llm_binding_api_key) = {
        let has_provider = llm_provider_id
            .as_ref()
            .map_or(false, |pid| !pid.is_empty());
        if has_provider {
            let pid = llm_provider_id.as_ref().unwrap();
            let config_repo = ConfigRepository::new();
            match config_repo.load() {
                Ok(cfg) => match cfg.find_provider(pid) {
                    Some(p) => {
                        let binding = match p.provider_type {
                            ProviderType::OpenAI
                            | ProviderType::Groq
                            | ProviderType::OpenRouter
                            | ProviderType::Custom
                            | ProviderType::Local => "openai",
                            ProviderType::Anthropic => "anthropic",
                            ProviderType::Ollama => "ollama",
                        };
                        // For Local provider, use the bridge host URL
                        let host = if matches!(p.provider_type, ProviderType::Local) {
                            "http://host.docker.internal:11337/v1".to_string()
                        } else {
                            p.base_url.clone()
                        };
                        // Look up the API key for non-Ollama providers
                        let api_key = if binding != "ollama" {
                            let secret_ref = crate::secrets::refs::summary_provider_key(pid);
                            store
                                .get(&secret_ref)
                                .await
                                .ok()
                                .flatten()
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        (binding.to_string(), host, api_key)
                    }
                    None => (
                        "openai".into(),
                        "http://host.docker.internal:11337/v1".into(),
                        String::new(),
                    ),
                },
                Err(_) => (
                    "openai".into(),
                    "http://host.docker.internal:11337/v1".into(),
                    String::new(),
                ),
            }
        } else {
            (
                "openai".into(),
                "http://host.docker.internal:11337/v1".into(),
                String::new(),
            )
        }
    };

    let dot_env_path = runtime_dir.join(".env");

    let neo4j_bytes: [u8; 32] = rand::thread_rng().gen();
    let neo4j_password = format!(
        "poly-{}",
        neo4j_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    );
    let dot_env_content = generate_kg_env_content(
        &neo4j_password,
        &lightrag_api_key,
        &llm_binding,
        &llm_binding_host,
        &llm_binding_api_key,
        &llm_model,
    );

    std::fs::write(&dot_env_path, dot_env_content).map_err(|e| {
        let msg = format!("Failed to write .env: {}", e);
        emit_setup_progress(&app, &msg, "error", "error");
        msg
    })?;
    info!("Wrote .env at {:?}", dot_env_path);
    emit_setup_progress(
        &app,
        ".env written with secure credentials and LightRAG config",
        "success",
        "env-file",
    );

    // ── Copy Poly prompt profile to runtime prompts directory ───────
    emit_setup_progress(
        &app,
        "Installing Poly meeting ontology prompt profile...",
        "info",
        "prompt-profile",
    );
    let prompts_dir = runtime_dir.join("data").join("prompts").join("entity_type");
    std::fs::create_dir_all(&prompts_dir).map_err(|e| {
        let msg = format!("Failed to create prompts directory: {}", e);
        emit_setup_progress(&app, &msg, "error", "error");
        msg
    })?;
    let prompt_source = {
        let compose_parent = compose_source_path
            .parent()
            .ok_or_else(|| "Failed to resolve compose source directory".to_string())?;
        // Production bundle layout: prompts/ is sibling to docker-compose.kg.yml
        let prod_candidate = compose_parent
            .join("prompts")
            .join("entity_type")
            .join("poly_meeting_ontology.yml");
        // Dev layout: prompts/ lives under frontend/resources/ relative to project root
        let dev_candidate = compose_parent
            .join("frontend")
            .join("resources")
            .join("prompts")
            .join("entity_type")
            .join("poly_meeting_ontology.yml");
        if prod_candidate.exists() {
            prod_candidate
        } else {
            dev_candidate
        }
    };
    if prompt_source.exists() {
        let prompt_dest = prompts_dir.join("poly_meeting_ontology.yml");
        std::fs::copy(&prompt_source, &prompt_dest).map_err(|e| {
            let msg = format!("Failed to copy prompt profile: {}", e);
            emit_setup_progress(&app, &msg, "error", "error");
            msg
        })?;
        info!("Copied Poly prompt profile to {:?}", prompt_dest);
        emit_setup_progress(
            &app,
            "Poly meeting ontology prompt profile installed",
            "success",
            "prompt-profile",
        );
    } else {
        let msg = format!(
            "Prompt profile not found at {:?}. Ensure prompts/entity_type/poly_meeting_ontology.yml is bundled.",
            prompt_source
        );
        emit_setup_progress(&app, &msg, "error", "error");
        return Err(msg);
    }

    // ── Run docker compose up ──────────────────────────────────────
    emit_setup_progress(
        &app,
        "Starting Docker containers (this may take a minute)...",
        "info",
        "compose-up",
    );

    // Bring up all services, force-recreate lightrag so it always
    // picks up the freshly-written .env config (LIGHTRAG_API_KEY, etc.)
    // Docker Compose automatically reads .env from the same directory
    // as the compose file — no explicit --env-file needed.
    let output = process_path::command("docker")
        .args([
            "compose",
            "-f",
            &compose_path.to_string_lossy(),
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

    let health_client = reqwest::Client::new();
    let health_url = "http://localhost:9621/health";
    let max_attempts: u32 = 40;

    for attempt in 1..=max_attempts {
        let req = health_client
            .get(health_url)
            .timeout(Duration::from_secs(5))
            .header("X-API-Key", &lightrag_api_key);

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

    let embedding_model = "BAAI/bge-m3".to_string();

    let profile = KnowledgeGraphProfile {
        id: "local-lightrag".to_string(),
        name: "Local LightRAG".to_string(),
        kind: ProfileKind::Local,
        embedding: EmbeddingConfig {
            provider: "openai".to_string(),
            model: embedding_model,
            dimensions: 1024,
        },
        lightrag_url: "http://localhost:9621".to_string(),
        // Key is already stored in SecretStore (created or verified above).
        // Pass None so save_kg_settings leaves the existing secret untouched.
        api_key: None,
        notes: Some("Auto-provisioned by 'Set it up for me'".to_string()),
        has_secret: false,
        api_key_masked_hint: None,
        llm_model: llm_model.clone(),
        llm_provider_id: llm_provider_id.clone(),
    };

    let config_repo = ConfigRepository::new();
    let mut settings = load_kg_settings(&config_repo, &store).await?;
    settings.profiles.retain(|p| p.id != "local-lightrag");
    settings.profiles.push(profile.clone());
    settings.active_profile = KnowledgeGraphSelection::Profile("local-lightrag".to_string());
    save_kg_settings(&config_repo, &store, &settings).await?;

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

    // Return a status message that does NOT include the raw API key.
    Ok(format!(
        "Knowledge graph stack is running.\n\
         LightRAG: http://localhost:9621\n\
         Profile: {} (active)",
        profile.name,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_graph::config::{
        KnowledgeGraphProfile, KnowledgeGraphSelection, ProfileKind,
    };
    use crate::secrets::types::{SecretRef, SecretStoreError};
    use async_trait::async_trait;
    use httpmock::MockServer;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Simple in-memory SecretStore for tests.
    struct TestSecretStore {
        data: Mutex<HashMap<String, String>>,
    }

    impl TestSecretStore {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl SecretStore for TestSecretStore {
        async fn get(&self, key: &SecretRef) -> Result<Option<String>, SecretStoreError> {
            Ok(self.data.lock().unwrap().get(key.as_str()).cloned())
        }

        async fn set(&self, key: &SecretRef, value: &str) -> Result<(), SecretStoreError> {
            self.data
                .lock()
                .unwrap()
                .insert(key.as_str().to_string(), value.to_string());
            Ok(())
        }

        async fn delete(&self, key: &SecretRef) -> Result<(), SecretStoreError> {
            self.data.lock().unwrap().remove(key.as_str());
            Ok(())
        }

        async fn exists(&self, key: &SecretRef) -> Result<bool, SecretStoreError> {
            Ok(self.data.lock().unwrap().contains_key(key.as_str()))
        }
    }

    fn test_config_repo() -> (ConfigRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("poly.yml");
        (ConfigRepository::with_path(path), dir)
    }

    // ── load_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn load_returns_defaults_when_no_config_file() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let settings = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(settings.profiles.len(), 0);
        assert_eq!(settings.active_profile, KnowledgeGraphSelection::None);
    }

    // ── save_kg_settings tests ────────────────────────────────────────

    #[tokio::test]
    async fn save_accepts_empty_profiles() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let empty = KnowledgeGraphSettings {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        };
        assert!(save_kg_settings(&repo, &store, &empty).await.is_ok());
        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(loaded.profiles.len(), 0);
    }

    #[tokio::test]
    async fn save_rejects_empty_profile_name() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&repo, &store, &invalid).await.unwrap_err();
        assert!(err.contains("Profile name cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_empty_lightrag_url() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "x".into(),
                lightrag_url: "".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        let err = save_kg_settings(&repo, &store, &invalid).await.unwrap_err();
        assert!(err.contains("LightRAG URL cannot be empty"));
    }

    #[tokio::test]
    async fn save_rejects_missing_active_profile() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let invalid = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "a".into(),
                name: "Alpha".into(),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("b".into()),
        };
        let err = save_kg_settings(&repo, &store, &invalid).await.unwrap_err();
        assert!(err.contains("Active profile 'b' not found"));
    }

    // ── YAML + SecretStore roundtrip ────────────────────────────────────

    #[tokio::test]
    async fn kg_settings_roundtrip_uses_yaml_and_secret_store() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        // Save two profiles: one with secret, one without.
        let settings = KnowledgeGraphSettings {
            profiles: vec![
                KnowledgeGraphProfile {
                    id: "prod-1".into(),
                    name: "Production".into(),
                    kind: ProfileKind::Remote,
                    lightrag_url: "https://kg.example.com".into(),
                    api_key: Some("sk-secret-prod-key-12345".into()),
                    notes: Some("production cluster".into()),
                    ..Default::default()
                },
                KnowledgeGraphProfile {
                    id: "local-dev".into(),
                    name: "Local Dev".into(),
                    kind: ProfileKind::Local,
                    lightrag_url: "http://localhost:9621".into(),
                    api_key: None,
                    notes: None,
                    ..Default::default()
                },
            ],
            active_profile: KnowledgeGraphSelection::Profile("prod-1".into()),
        };
        save_kg_settings(&repo, &store, &settings)
            .await
            .expect("save should succeed");

        // Load back — verify secret enrichment.
        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(
            loaded.active_profile,
            KnowledgeGraphSelection::Profile("prod-1".into())
        );

        let prod = loaded.profiles.iter().find(|p| p.id == "prod-1").unwrap();
        assert_eq!(prod.name, "Production");
        assert_eq!(prod.lightrag_url, "https://kg.example.com");
        assert!(prod.api_key.is_none(), "api_key must be None");
        assert!(
            prod.has_secret,
            "has_secret must be true for profile with stored key"
        );
        assert!(
            prod.api_key_masked_hint.is_some(),
            "masked_hint must be present"
        );
        assert!(
            prod.api_key_masked_hint
                .as_deref()
                .unwrap()
                .starts_with("sk-"),
            "masked_hint should start with prefix"
        );

        let local = loaded
            .profiles
            .iter()
            .find(|p| p.id == "local-dev")
            .unwrap();
        assert_eq!(local.name, "Local Dev");
        assert!(local.api_key.is_none());
        assert!(!local.has_secret, "has_secret must be false");
        assert!(
            local.api_key_masked_hint.is_none(),
            "masked_hint must be None for profile without key"
        );

        // Verify YAML on disk has no API key.
        let yaml = std::fs::read_to_string(repo.path()).unwrap();
        assert!(yaml.contains("prod-1"), "YAML should contain profile id");
        assert!(
            !yaml.contains("sk-secret-prod"),
            "YAML must not contain raw API key"
        );
    }

    #[tokio::test]
    async fn remove_profile_deletes_secret_from_store() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        // Save a profile with a secret.
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "temp".into(),
                name: "Temp".into(),
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("temp-key".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        // Verify secret exists.
        let secret_ref = knowledge_graph_profile_key("temp");
        assert!(
            store.exists(&secret_ref).await.unwrap(),
            "secret should exist after save"
        );

        // Remove the profile.
        let pruned = KnowledgeGraphSettings {
            profiles: vec![],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &pruned).await.unwrap();

        // Verify secret is gone.
        assert!(
            !store.exists(&secret_ref).await.unwrap(),
            "secret should be deleted when profile is removed"
        );
    }

    #[tokio::test]
    async fn clear_api_key_via_empty_string_deletes_secret() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        // Save with a key.
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "x".into(),
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("old-key".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        // Save again with empty key string.
        let cleared = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "x".into(),
                name: "x".into(),
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &cleared).await.unwrap();

        let secret_ref = knowledge_graph_profile_key("x");
        assert!(
            !store.exists(&secret_ref).await.unwrap(),
            "secret should be deleted when cleared with empty string"
        );
    }

    #[tokio::test]
    async fn api_key_none_leaves_existing_secret_untouched() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        // Save with a key.
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "y".into(),
                name: "y".into(),
                lightrag_url: "http://localhost:9621".into(),
                api_key: Some("persistent-key".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        // Save again with api_key: None (meaning "don't change").
        let unchanged = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "y".into(),
                name: "y updated".into(),
                lightrag_url: "http://localhost:9621".into(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &unchanged).await.unwrap();

        let secret_ref = knowledge_graph_profile_key("y");
        let stored = store.get(&secret_ref).await.unwrap();
        assert_eq!(
            stored.as_deref(),
            Some("persistent-key"),
            "existing secret should survive when api_key is None"
        );
    }

    // ── Profile CRUD roundtrip ──────────────────────────────────────

    #[tokio::test]
    async fn profile_add_update_delete_roundtrip() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let initial = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(initial.profiles.len(), 0);

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "remote-1".into(),
                name: "cloud kg".into(),
                kind: ProfileKind::Remote,
                lightrag_url: "https://kg.example.com".into(),
                api_key: Some("remote-key".into()),
                notes: Some("team server".into()),
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].id, "remote-1");

        let mut updated = loaded.clone();
        if let Some(p) = updated.profiles.iter_mut().find(|p| p.id == "remote-1") {
            p.name = "cloud kg v2".into();
            p.lightrag_url = "https://kg2.example.com".into();
        }
        save_kg_settings(&repo, &store, &updated).await.unwrap();

        let loaded2 = load_kg_settings(&repo, &store).await.unwrap();
        let p = loaded2
            .profiles
            .iter()
            .find(|p| p.id == "remote-1")
            .unwrap();
        assert_eq!(p.name, "cloud kg v2");
        assert_eq!(p.lightrag_url, "https://kg2.example.com");
        assert!(p.api_key.is_none());

        let mut pruned = loaded2.clone();
        pruned.profiles.retain(|p| p.id != "remote-1");
        save_kg_settings(&repo, &store, &pruned).await.unwrap();

        let loaded3 = load_kg_settings(&repo, &store).await.unwrap();
        assert_eq!(loaded3.profiles.len(), 0);
    }

    // ── Health check tests with mock HTTP ───────────────────────────

    #[tokio::test]
    async fn health_test_healthy_with_mock_server() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();
        let server = MockServer::start();

        // Persist a profile with secret via SecretStore directly,
        // then save profile fields to YAML.
        let secret_ref = knowledge_graph_profile_key("health-1");
        store.set(&secret_ref, "secret").await.unwrap();

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "health-1".into(),
                name: "mock".into(),
                kind: ProfileKind::Local,
                lightrag_url: server.base_url(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::Profile("health-1".into()),
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/health")
                .header("X-API-Key", "secret");
            then.status(200)
                .json_body(json!({"healthy": true, "version": "2.0.0"}));
        });

        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        let profile = loaded.profiles.iter().find(|p| p.id == "health-1").unwrap();
        let actual_key = store.get(&secret_ref).await.unwrap();
        let provider = LightRagProvider::new(&profile.lightrag_url, actual_key).unwrap();
        let health = provider.health().await.unwrap();

        mock.assert();
        assert!(health.healthy);
        assert_eq!(health.version.as_deref(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn health_test_unhealthy_with_mock_server() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();
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
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/health");
            then.status(500).body("internal error");
        });

        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        let profile = loaded
            .profiles
            .iter()
            .find(|p| p.id == "unhealthy-1")
            .unwrap();
        let result = LightRagProvider::new(&profile.lightrag_url, None)
            .unwrap()
            .health()
            .await;

        mock.assert();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("internal error"),
            "expected error to mention 500 or 'internal error', got: {err}"
        );
    }

    #[tokio::test]
    async fn health_test_profile_not_found() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        let found = loaded.profiles.iter().find(|p| p.id == "nonexistent");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn health_test_connection_refused() {
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "dead".into(),
                name: "unreachable".into(),
                kind: ProfileKind::Local,
                lightrag_url: "http://127.0.0.1:19999".into(),
                api_key: None,
                ..Default::default()
            }],
            active_profile: KnowledgeGraphSelection::None,
        };
        save_kg_settings(&repo, &store, &settings).await.unwrap();

        let loaded = load_kg_settings(&repo, &store).await.unwrap();
        let profile = loaded.profiles.iter().find(|p| p.id == "dead").unwrap();
        let result = LightRagProvider::new(&profile.lightrag_url, None)
            .unwrap()
            .health()
            .await;

        assert!(result.is_err(), "expected connection error");
    }

    // ── Setup env generation test ─────────────────────────────────

    #[tokio::test]
    async fn kg_setup_generates_env_from_yaml_and_secret_store_without_returning_raw_key() {
        // Given: a fresh config repository and a secret store pre-populated
        // with the local-lightrag API key (as api_setup_local_knowledge_graph
        // would do during setup).
        let (repo, _dir) = test_config_repo();
        let store = TestSecretStore::new();

        let secret_ref = knowledge_graph_profile_key("local-lightrag");
        let raw_key = "poly-lrag-abc123secret456";
        store.set(&secret_ref, raw_key).await.unwrap();

        // Save a local-lightrag profile WITHOUT the raw key (it is already
        // in SecretStore).  This matches what api_setup_local_knowledge_graph
        // does after this change: api_key is None, key lives in SecretStore.
        let settings = KnowledgeGraphSettings {
            profiles: vec![KnowledgeGraphProfile {
                id: "local-lightrag".into(),
                name: "Local LightRAG".into(),
                kind: ProfileKind::Local,
                embedding: EmbeddingConfig {
                    provider: "openai".into(),
                    model: "BAAI/bge-m3".into(),
                    dimensions: 1024,
                },
                lightrag_url: "http://localhost:9621".into(),
                api_key: None,
                notes: Some("Auto-provisioned".into()),
                has_secret: false,
                api_key_masked_hint: None,
                llm_model: "qwen3:30b-a3b".into(),
                llm_provider_id: None,
            }],
            active_profile: KnowledgeGraphSelection::Profile("local-lightrag".into()),
        };
        save_kg_settings(&repo, &store, &settings)
            .await
            .expect("save should succeed");

        // When: we load the settings back
        let loaded = load_kg_settings(&repo, &store).await.unwrap();

        // Then: no raw API key is ever returned to callers
        assert_eq!(loaded.profiles.len(), 1);
        let profile = &loaded.profiles[0];
        assert!(
            profile.api_key.is_none(),
            "api_key must be None — raw key must never leak through settings"
        );
        assert!(
            profile.has_secret,
            "has_secret must be true — key exists in SecretStore"
        );
        assert!(
            profile.api_key_masked_hint.is_some(),
            "masked_hint must be present for secret-backed profile"
        );
        assert!(
            profile
                .api_key_masked_hint
                .as_deref()
                .unwrap()
                .starts_with("pol"),
            "masked hint should show key prefix"
        );

        // And: YAML on disk must not contain the raw key
        let yaml = std::fs::read_to_string(repo.path()).unwrap();
        assert!(
            !yaml.contains(raw_key),
            "YAML config must not contain raw API key"
        );
        assert!(
            yaml.contains("local-lightrag"),
            "YAML should contain the profile id"
        );

        // And: the key is still in SecretStore
        let stored = store.get(&secret_ref).await.unwrap();
        assert_eq!(stored.as_deref(), Some(raw_key));
    }

    // ── generate_kg_env_content tests ─────────────────────────────

    #[test]
    fn test_kg_env_content_uses_openai_llm_binding() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("LLM_BINDING=openai"));
    }

    #[test]
    fn test_kg_env_content_uses_openai_embedding_binding() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("EMBEDDING_BINDING=openai"));
    }

    #[test]
    fn test_kg_env_content_has_bridge_host() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("LLM_BINDING_HOST=http://host.docker.internal:11337/v1"));
        assert!(content.contains("EMBEDDING_BINDING_HOST=http://host.docker.internal:11337/v1"));
    }

    #[test]
    fn test_kg_env_content_uses_llm_binding_api_key() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("LLM_BINDING_API_KEY=not-needed"));
        assert!(!content.contains("OPENAI_API_KEY="));
    }

    #[test]
    fn test_kg_env_content_no_ollama_defaults() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(
            !content.contains("ollama"),
            "generated env must not contain any ollama reference"
        );
        assert!(
            !content.contains("11434"),
            "generated env must not reference ollama port 11434"
        );
    }

    #[test]
    fn test_kg_env_content_ollama_binding_respected_when_explicit() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "ollama",
            "http://host.docker.internal:11434",
            "",
            "llama3:8b",
        );
        assert!(content.contains("LLM_BINDING=ollama"));
        assert!(content.contains("LLM_BINDING_HOST=http://host.docker.internal:11434"));
    }

    #[test]
    fn test_kg_env_content_has_embedding_dim_1024() {
        let content = generate_kg_env_content(
            "secret",
            "poly-key",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("EMBEDDING_DIM=1024"));
        assert!(content.contains("EMBEDDING_MODEL=bge-m3"));
    }

    #[test]
    fn test_kg_env_content_includes_neo4j_and_lightrag_keys() {
        let content = generate_kg_env_content(
            "poly-abc123",
            "poly-lrag-def456",
            "openai",
            "http://host.docker.internal:11337/v1",
            "not-needed",
            "test-model",
        );
        assert!(content.contains("NEO4J_AUTH=neo4j/poly-abc123"));
        assert!(content.contains("LIGHTRAG_API_KEY=poly-lrag-def456"));
    }

    // ── Dependency check tests ──────────────────────────────────

    #[test]
    fn test_setup_deps_result_has_no_ollama_field() {
        let result = SetupDependencyResult {
            docker: DependencyInfo {
                installed: true,
                version: Some("v1".into()),
            },
            platform: "macos".into(),
        };
        assert!(result.docker.installed);
        assert_eq!(result.platform, "macos");
        // Compile-time check: struct has no ollama field
    }
}
