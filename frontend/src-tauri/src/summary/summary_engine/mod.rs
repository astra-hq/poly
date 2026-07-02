// Local AI summary engine module
// Provides local LLM inference via llama-helper sidecar

pub mod client;
pub mod commands;
pub mod model_manager;
pub mod models;
pub mod sidecar;

// Re-export commonly used types
pub use client::{
    force_shutdown_sidecar, generate_with_builtin, is_sidecar_healthy, shutdown_sidecar_gracefully,
};
pub use commands::{
    __cmd__add_custom_model, __cmd__local_ai_cancel_download, __cmd__local_ai_delete_model,
    __cmd__local_ai_download_model, __cmd__local_ai_get_available_summary_model,
    __cmd__local_ai_get_model_info, __cmd__local_ai_get_recommended_model,
    __cmd__local_ai_is_model_ready, __cmd__local_ai_list_models, __cmd__verify_hf_repo,
    __tauri_command_name_add_custom_model, __tauri_command_name_local_ai_cancel_download,
    __tauri_command_name_local_ai_delete_model, __tauri_command_name_local_ai_download_model,
    __tauri_command_name_local_ai_get_available_summary_model,
    __tauri_command_name_local_ai_get_model_info,
    __tauri_command_name_local_ai_get_recommended_model,
    __tauri_command_name_local_ai_is_model_ready, __tauri_command_name_local_ai_list_models,
    __tauri_command_name_verify_hf_repo, add_custom_model, init_model_manager,
    local_ai_cancel_download, local_ai_delete_model, local_ai_download_model,
    local_ai_get_available_summary_model, local_ai_get_model_info, local_ai_get_recommended_model,
    local_ai_is_model_ready, local_ai_list_models, verify_hf_repo, ModelManagerState,
};
pub use model_manager::{ModelInfo, ModelStatus};
pub use models::{
    get_all_models, get_available_embedding_models, get_available_models, get_default_model,
    get_embedding_models_directory, get_model_by_name, get_model_by_name_any,
    get_summary_models_directory, refresh_custom_registry_cache, CustomModelEntry, GgufCandidate,
    HfRepoVerification, ModelDef, ModelType, RegistrySource,
};
