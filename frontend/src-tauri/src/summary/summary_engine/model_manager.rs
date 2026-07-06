// Model manager for built-in AI models - handles downloads and lifecycle
// Follows the same pattern as whisper_engine/whisper_engine.rs for consistency

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::RwLock;
use tokio::time::timeout;

use super::models::{get_all_models, get_model_by_name_any, ModelType};

// ============================================================================
// Model Status Types
// ============================================================================

/// Detailed download progress info (MB-based with speed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Total file size in bytes
    pub total_bytes: u64,
    /// Downloaded in MB (for display)
    pub downloaded_mb: f64,
    /// Total size in MB (for display)
    pub total_mb: f64,
    /// Download speed in MB/s
    pub speed_mbps: f64,
    /// Percentage complete (0-100)
    pub percent: u8,
}

impl DownloadProgress {
    pub fn new(downloaded: u64, total: u64, speed_mbps: f64) -> Self {
        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u8
        } else {
            0
        };
        Self {
            downloaded_bytes: downloaded,
            total_bytes: total,
            downloaded_mb: downloaded as f64 / (1024.0 * 1024.0),
            total_mb: total as f64 / (1024.0 * 1024.0),
            speed_mbps,
            percent,
        }
    }
}

/// Model status in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelStatus {
    /// Model is not yet downloaded
    NotDownloaded,

    /// Model is currently being downloaded (progress 0-100)
    Downloading { progress: u8 },

    /// Model is downloaded and ready to use
    Available,

    /// Model file is corrupted and needs redownload
    Corrupted {
        file_size: u64,
        expected_min_size: u64,
    },

    /// Error occurred with the model
    Error(String),
}

/// Model information for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model name (e.g., "gemma3:1b")
    pub name: String,

    /// Display name for UI
    pub display_name: String,

    /// Current status
    pub status: ModelStatus,

    /// File path (if available)
    pub path: PathBuf,

    /// Size in MB
    pub size_mb: u64,

    /// Context window size in tokens
    pub context_size: u32,

    /// Description
    pub description: String,

    /// GGUF filename on disk
    pub gguf_file: String,

    /// Model type (summary or embedding)
    pub model_type: ModelType,
}
// ============================================================================

pub struct ModelManager {
    /// Directory where models are stored
    models_dir: PathBuf,

    /// Currently available models with their status
    available_models: Arc<RwLock<HashMap<String, ModelInfo>>>,

    /// Active downloads (model names)
    active_downloads: Arc<RwLock<HashSet<String>>>,

    /// Cancellation flag for current download
    cancel_download_flag: Arc<RwLock<Option<String>>>,
}

impl ModelManager {
    /// Create a new model manager with the given models directory.
    ///
    /// For production use, the caller must provide a path resolved via
    /// Tauri's `app.path().app_data_dir()` API. A convenience initializer
    /// that uses a hardcoded system path is intentionally NOT provided —
    /// this prevents writable model data from ending up in the read-only
    /// resource directory or an incorrect platform-specific default.
    pub fn new_with_models_dir(models_dir: PathBuf) -> Result<Self> {
        log::info!(
            "Local AI ModelManager using directory: {}",
            models_dir.display()
        );

        Ok(Self {
            models_dir,
            available_models: Arc::new(RwLock::new(HashMap::new())),
            active_downloads: Arc::new(RwLock::new(HashSet::new())),
            cancel_download_flag: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize and scan for existing models
    pub async fn init(&self) -> Result<()> {
        // Create base models directory and type subdirectories
        if !self.models_dir.exists() {
            fs::create_dir_all(&self.models_dir).await?;
            log::info!("Created models directory: {}", self.models_dir.display());
        }

        for subdir in &["summary", "embedding"] {
            let type_dir = self.models_dir.join(subdir);
            if !type_dir.exists() {
                fs::create_dir_all(&type_dir).await?;
            }
        }

        // Scan for existing models
        self.scan_models().await?;

        Ok(())
    }

    /// Scan models directory and update status
    pub async fn scan_models(&self) -> Result<()> {
        let start = std::time::Instant::now();

        log::info!(
            "Starting model scan in directory: {}",
            self.models_dir.display()
        );

        let model_defs = get_all_models();
        let mut models_map = HashMap::new();

        for model_def in model_defs {
            let model_path = self
                .models_dir
                .join(model_def.subdir())
                .join(&model_def.gguf_file);
            log::debug!(
                "Checking model '{}' at path: {}",
                model_def.name,
                model_path.display()
            );

            let is_actively_downloading = {
                let active = self.active_downloads.read().await;
                active.contains(&model_def.name)
            };

            // If actively downloading, preserve existing status from memory
            if is_actively_downloading {
                let existing_info = {
                    let models = self.available_models.read().await;
                    models.get(&model_def.name).cloned()
                };

                if let Some(info) = existing_info {
                    // Preserve existing status (should be Downloading)
                    models_map.insert(model_def.name.clone(), info);
                    log::debug!(
                        "Model '{}': Preserving Downloading status during scan",
                        model_def.name
                    );
                    continue;
                }
            }

            let status = if model_path.exists() {
                // Check if file size matches expected size (basic validation)
                match fs::metadata(&model_path).await {
                    Ok(metadata) => {
                        let file_size_mb = metadata.len() / (1024 * 1024);

                        // Allow 10% variance for file size check
                        let expected_min = (model_def.size_mb as f64 * 0.9) as u64;
                        let expected_max = (model_def.size_mb as f64 * 1.1) as u64;

                        log::info!(
                            "Model '{}': found {} MB (expected {}-{} MB)",
                            model_def.name,
                            file_size_mb,
                            expected_min,
                            expected_max
                        );

                        if file_size_mb >= expected_min && file_size_mb <= expected_max {
                            log::info!("Model '{}': AVAILABLE", model_def.name);
                            ModelStatus::Available
                        } else {
                            log::warn!(
                                "Model '{}': CORRUPTED (size mismatch: {} MB, expected {} MB)",
                                model_def.name,
                                file_size_mb,
                                model_def.size_mb
                            );
                            ModelStatus::Corrupted {
                                file_size: file_size_mb,
                                expected_min_size: expected_min,
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Model '{}': Failed to read metadata: {}", model_def.name, e);
                        ModelStatus::Error(format!("Failed to read metadata: {}", e))
                    }
                }
            } else {
                log::debug!("Model '{}': NOT FOUND", model_def.name);
                ModelStatus::NotDownloaded
            };

            let model_info = ModelInfo {
                name: model_def.name.clone(),
                display_name: model_def.display_name.clone(),
                status,
                path: model_path,
                size_mb: model_def.size_mb,
                context_size: model_def.context_size,
                description: model_def.description.clone(),
                gguf_file: model_def.gguf_file.clone(),
                model_type: model_def.model_type.clone(),
            };

            models_map.insert(model_def.name.clone(), model_info);
        }

        let model_count = models_map.len();

        let mut models = self.available_models.write().await;
        *models = models_map;

        let elapsed = start.elapsed();
        log::info!(
            "Model scan complete: {} models checked in {:?}",
            model_count,
            elapsed
        );
        Ok(())
    }

    /// Get list of all models with their status
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.available_models
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get info for a specific model
    pub async fn get_model_info(&self, model_name: &str) -> Option<ModelInfo> {
        self.available_models.read().await.get(model_name).cloned()
    }

    /// Check if a model is ready to use
    /// If refresh=true, scans filesystem before checking (slower but accurate)
    pub async fn is_model_ready(&self, model_name: &str, refresh: bool) -> bool {
        if refresh {
            if let Err(e) = self.scan_models().await {
                log::error!("Failed to scan models: {}", e);
                return false;
            }
        }

        if let Some(info) = self.get_model_info(model_name).await {
            info.status == ModelStatus::Available
        } else {
            false
        }
    }

    /// Download a model with simple percentage callback (backward compatible)
    pub async fn download_model(
        &self,
        model_name: &str,
        progress_callback: Option<Box<dyn Fn(u8) + Send>>,
    ) -> Result<()> {
        // Wrap the simple callback to use detailed progress internally
        let detailed_callback: Option<Box<dyn Fn(DownloadProgress) + Send>> = progress_callback
            .map(|cb| {
                Box::new(move |p: DownloadProgress| cb(p.percent))
                    as Box<dyn Fn(DownloadProgress) + Send>
            });
        self.download_model_detailed(model_name, detailed_callback)
            .await
    }

    /// Download a model with detailed progress (MB, speed, etc.)
    pub async fn download_model_detailed(
        &self,
        model_name: &str,
        progress_callback: Option<Box<dyn Fn(DownloadProgress) + Send>>,
    ) -> Result<()> {
        log::info!("Starting download for model: {}", model_name);

        // Check if already downloading
        {
            let active = self.active_downloads.read().await;
            if active.contains(model_name) {
                log::warn!("Download already in progress for model: {}", model_name);
                return Err(anyhow!("Download already in progress"));
            }
        }

        // Get model definition
        let model_def = get_model_by_name_any(model_name)
            .ok_or_else(|| anyhow!("Unknown model: {}", model_name))?;

        // Ensure model exists in available_models map (custom models added after
        // the last scan are in CUSTOM_REGISTRY but not yet in the manager map)
        {
            let mut models = self.available_models.write().await;
            if !models.contains_key(model_name) {
                let model_path = self
                    .models_dir
                    .join(model_def.subdir())
                    .join(&model_def.gguf_file);
                let model_info = ModelInfo {
                    name: model_def.name.clone(),
                    display_name: model_def.display_name.clone(),
                    status: ModelStatus::NotDownloaded,
                    path: model_path,
                    size_mb: model_def.size_mb,
                    context_size: model_def.context_size,
                    description: model_def.description.clone(),
                    gguf_file: model_def.gguf_file.clone(),
                    model_type: model_def.model_type.clone(),
                };
                models.insert(model_name.to_string(), model_info);
            }
        }

        // Add to active downloads
        {
            let mut active = self.active_downloads.write().await;
            active.insert(model_name.to_string());
        }

        // Clear cancellation flag
        {
            let mut cancel_flag = self.cancel_download_flag.write().await;
            *cancel_flag = None;
        }

        // Update status to downloading
        {
            let mut models = self.available_models.write().await;
            if let Some(model_info) = models.get_mut(model_name) {
                model_info.status = ModelStatus::Downloading { progress: 0 };
            }
        }

        let file_path = self
            .models_dir
            .join(model_def.subdir())
            .join(&model_def.gguf_file);

        // Check if model already exists and is valid (skip re-download)
        if file_path.exists() {
            if let Ok(metadata) = fs::metadata(&file_path).await {
                let file_size_mb = metadata.len() / (1024 * 1024);
                let expected_min = (model_def.size_mb as f64 * 0.9) as u64;
                let expected_max = (model_def.size_mb as f64 * 1.1) as u64;

                if file_size_mb >= expected_min && file_size_mb <= expected_max {
                    log::info!(
                        "Model '{}' already exists and is valid ({} MB), skipping download",
                        model_name,
                        file_size_mb
                    );

                    // Update status to available
                    {
                        let mut models = self.available_models.write().await;
                        if let Some(model_info) = models.get_mut(model_name) {
                            model_info.status = ModelStatus::Available;
                        }
                    }

                    // Remove from active downloads
                    {
                        let mut active = self.active_downloads.write().await;
                        active.remove(model_name);
                    }

                    // Report 100% progress
                    if let Some(ref callback) = progress_callback {
                        let total = metadata.len();
                        callback(DownloadProgress::new(total, total, 0.0));
                    }

                    return Ok(());
                } else if file_size_mb > expected_max {
                    // File is LARGER than expected - possibly corrupted or wrong file
                    // Delete and re-download in this case
                    log::warn!(
                        "Model '{}' exists but is too large ({} MB, expected max {} MB), deleting and re-downloading",
                        model_name,
                        file_size_mb,
                        expected_max
                    );
                    if let Err(e) = fs::remove_file(&file_path).await {
                        log::warn!("Failed to delete oversized model file: {}", e);
                    }
                } else {
                    // File is SMALLER than expected - likely partial download
                    // DON'T DELETE - let resume logic handle it
                    log::info!(
                        "Model '{}' exists but is incomplete ({} MB, expected min {} MB), will resume download",
                        model_name,
                        file_size_mb,
                        expected_min
                    );
                    // Continue to download/resume logic below
                }
            }
        }

        log::info!("Downloading from: {}", model_def.download_url);
        log::info!("Saving to: {}", file_path.display());

        // Create models directory if needed
        if !self.models_dir.exists() {
            fs::create_dir_all(&self.models_dir).await?;
        }

        // Check for existing partial download to resume
        let existing_size: u64 = if file_path.exists() {
            fs::metadata(&file_path).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        // Download the file with optimized client settings
        let client = Client::builder()
            .tcp_nodelay(true) // Disable Nagle's algorithm for faster streaming
            .pool_max_idle_per_host(1) // Keep connection alive
            .timeout(Duration::from_secs(3600)) // 1 hour timeout for large files
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        // Build request with Range header if resuming
        let mut request = client.get(&model_def.download_url);
        if existing_size > 0 {
            log::info!(
                "Resuming download from byte {} ({:.1} MB)",
                existing_size,
                existing_size as f64 / (1024.0 * 1024.0)
            );
            request = request.header("Range", format!("bytes={}-", existing_size));
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow!("Failed to start download: {}", e))?;

        // Check response status - 200 OK (full download) or 206 Partial Content (resume)
        let (total_size, resuming) = if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            // Server supports resume - total size = existing + remaining
            let remaining = response.content_length().unwrap_or(0);
            log::info!(
                "Server supports resume, {} MB remaining",
                remaining / (1024 * 1024)
            );
            (existing_size + remaining, true)
        } else if response.status().is_success() {
            // Server doesn't support resume or fresh download
            if existing_size > 0 {
                log::warn!("Server doesn't support resume, starting fresh download");
            }
            (response.content_length().unwrap_or(0), false)
        } else {
            let mut active = self.active_downloads.write().await;
            active.remove(model_name);
            return Err(anyhow!(
                "Download failed with status: {}",
                response.status()
            ));
        };

        log::info!("Total size: {} MB", total_size / (1024 * 1024));

        // Open file for append if resuming, or create new
        let file = if resuming {
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(&file_path)
                .await
                .map_err(|e| anyhow!("Failed to open file for append: {}", e))?
        } else {
            fs::File::create(&file_path)
                .await
                .map_err(|e| anyhow!("Failed to create file: {}", e))?
        };

        // Use 8MB buffer to reduce disk I/O syscalls (major performance improvement)
        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);

        let mut downloaded: u64 = if resuming { existing_size } else { 0 };

        // Emit initial progress (showing resumed position if applicable)
        if let Some(ref callback) = progress_callback {
            callback(DownloadProgress::new(downloaded, total_size, 0.0));
        }
        log::info!(
            "Starting at {:.1} MB / {:.1} MB",
            downloaded as f64 / (1024.0 * 1024.0),
            total_size as f64 / (1024.0 * 1024.0)
        );

        let mut last_progress_percent = if total_size > 0 {
            ((downloaded as f64 / total_size as f64) * 100.0) as u8
        } else {
            0
        };
        let mut last_report_time = std::time::Instant::now();
        let mut bytes_since_last_report: u64 = 0;
        let download_start_time = std::time::Instant::now();
        let start_downloaded = downloaded;

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();

        loop {
            // Check for cancellation
            {
                let cancel_flag = self.cancel_download_flag.read().await;
                if cancel_flag.as_ref() == Some(&model_name.to_string()) {
                    log::info!("Download cancelled for model: {}", model_name);

                    // Flush and keep partial file for resume on next attempt
                    let _ = writer.flush().await;
                    drop(writer);

                    // Remove from active downloads
                    let mut active = self.active_downloads.write().await;
                    active.remove(model_name);

                    // Update status
                    {
                        let mut models = self.available_models.write().await;
                        if let Some(model_info) = models.get_mut(model_name) {
                            model_info.status = ModelStatus::NotDownloaded;
                        }
                    }

                    // Use special marker prefix to distinguish cancellation from other errors
                    return Err(anyhow!("CANCELLED: Download cancelled by user"));
                }
            }

            // Add per-chunk timeout (30 seconds) to detect stalled connections
            let next_result = timeout(Duration::from_secs(30), stream.next()).await;

            let chunk = match next_result {
                // Timeout - no data received for 30 seconds
                Err(_) => {
                    log::warn!(
                        "Download timeout for {}: no data received for 30 seconds",
                        model_name
                    );
                    let _ = writer.flush().await;

                    // Cleanup: Remove from active downloads
                    let mut active = self.active_downloads.write().await;
                    active.remove(model_name);

                    // Set model status to Error (NOT NotDownloaded) so UI can show retry button
                    {
                        let mut models = self.available_models.write().await;
                        if let Some(model_info) = models.get_mut(model_name) {
                            model_info.status = ModelStatus::Error(
                                "Download timeout - No data received for 30 seconds".to_string(),
                            );
                        }
                    }

                    return Err(anyhow!(
                        "Download timeout - No data received for 30 seconds"
                    ));
                }
                // Stream ended
                Ok(None) => break,
                // Got chunk result
                Ok(Some(chunk_result)) => {
                    match chunk_result {
                        Ok(c) => c,
                        // Detect error type for better user feedback
                        Err(e) => {
                            log::error!("Download error for {}: {:?}", model_name, e);
                            let _ = writer.flush().await;

                            // Cleanup: Remove from active downloads
                            let mut active = self.active_downloads.write().await;
                            active.remove(model_name);

                            // Categorize error for user-friendly message
                            let error_msg = if e.is_timeout() {
                                "Connection timeout - Check your internet"
                            } else if e.is_connect() {
                                "Connection failed - Check your internet"
                            } else if e.is_body() {
                                "Stream interrupted - Network unstable"
                            } else {
                                "Download error"
                            };

                            // Set model status to Error (NOT NotDownloaded) so UI can show retry button
                            {
                                let mut models = self.available_models.write().await;
                                if let Some(model_info) = models.get_mut(model_name) {
                                    model_info.status = ModelStatus::Error(error_msg.to_string());
                                }
                            }

                            return Err(anyhow!("{}: {}", error_msg, e));
                        }
                    }
                }
            };
            let chunk_len = chunk.len() as u64;
            writer
                .write_all(&chunk)
                .await
                .map_err(|e| anyhow!("Error writing to file: {}", e))?;

            downloaded += chunk_len;
            bytes_since_last_report += chunk_len;

            // Calculate progress
            let progress_percent = if total_size > 0 {
                let exact_percent = (downloaded as f64 / total_size as f64) * 100.0;
                exact_percent.min(100.0) as u8
            } else {
                0
            };

            let elapsed_since_report = last_report_time.elapsed();
            let is_download_complete = downloaded >= total_size;
            let should_report = progress_percent > last_progress_percent
                || is_download_complete  // Force report on completion
                || elapsed_since_report.as_millis() >= 500;

            if should_report {
                // Calculate speed based on bytes downloaded since last report
                let speed_mbps = if elapsed_since_report.as_secs_f64() > 0.0 {
                    (bytes_since_last_report as f64 / (1024.0 * 1024.0))
                        / elapsed_since_report.as_secs_f64()
                } else {
                    // Fallback to overall average speed
                    let total_elapsed = download_start_time.elapsed().as_secs_f64();
                    if total_elapsed > 0.0 {
                        ((downloaded - start_downloaded) as f64 / (1024.0 * 1024.0)) / total_elapsed
                    } else {
                        0.0
                    }
                };

                log::info!(
                    "Download: {:.1} MB / {:.1} MB ({:.1} MB/s)",
                    downloaded as f64 / (1024.0 * 1024.0),
                    total_size as f64 / (1024.0 * 1024.0),
                    speed_mbps
                );

                // Update status
                {
                    let mut models = self.available_models.write().await;
                    if let Some(model_info) = models.get_mut(model_name) {
                        model_info.status = ModelStatus::Downloading {
                            progress: if is_download_complete {
                                100
                            } else {
                                progress_percent
                            },
                        };
                    }
                }

                // Call progress callback with detailed info
                if let Some(ref callback) = progress_callback {
                    callback(DownloadProgress::new(downloaded, total_size, speed_mbps));
                }

                last_progress_percent = progress_percent;
                last_report_time = std::time::Instant::now();
                bytes_since_last_report = 0;
            }
        }

        writer.flush().await?;
        drop(writer);

        log::info!("Download completed for model: {}", model_name);

        {
            let mut models = self.available_models.write().await;
            if let Some(model_info) = models.get_mut(model_name) {
                model_info.status = ModelStatus::Downloading { progress: 100 };
            }
        }

        if let Some(ref callback) = progress_callback {
            callback(DownloadProgress::new(total_size, total_size, 0.0));
        }

        // Small delay to ensure UI receives 100% event
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Err(e) = self.validate_gguf_file(&file_path).await {
            log::error!("Downloaded file failed validation: {}", e);

            // Clean up invalid file
            let _ = fs::remove_file(&file_path).await;

            // Update status
            {
                let mut models = self.available_models.write().await;
                if let Some(model_info) = models.get_mut(model_name) {
                    model_info.status = ModelStatus::Error(format!("Validation failed: {}", e));
                }
            }

            // Remove from active downloads
            let mut active = self.active_downloads.write().await;
            active.remove(model_name);

            return Err(anyhow!("File validation failed: {}", e));
        }

        // Update status to available
        {
            let mut models = self.available_models.write().await;
            if let Some(model_info) = models.get_mut(model_name) {
                model_info.status = ModelStatus::Available;
                model_info.path = file_path.clone();
            }
        }

        // Remove from active downloads
        {
            let mut active = self.active_downloads.write().await;
            active.remove(model_name);
        }

        Ok(())
    }

    /// Validate that a file is a valid GGUF model
    async fn validate_gguf_file(&self, path: &PathBuf) -> Result<()> {
        let mut file = fs::File::open(path).await?;

        // Read first 4 bytes to check for GGUF magic number
        use tokio::io::AsyncReadExt;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).await?;

        // GGUF magic number is "GGUF" (0x47475546)
        if &magic == b"GGUF" {
            Ok(())
        } else if &magic == b"ggjt" || &magic == b"ggla" || &magic == b"ggml" {
            // Older formats (GGML, GGJT)
            Ok(())
        } else {
            Err(anyhow!(
                "Invalid model file: magic number {:?} doesn't match GGUF/GGML",
                magic
            ))
        }
    }

    /// Cancel an ongoing download
    pub async fn cancel_download(&self, model_name: &str) -> Result<()> {
        log::info!("Cancelling download for model: {}", model_name);

        // Set cancellation flag - download loop will detect this and handle cleanup
        {
            let mut cancel_flag = self.cancel_download_flag.write().await;
            *cancel_flag = Some(model_name.to_string());
        }

        // Note: active_downloads cleanup is handled by the download loop when it detects
        // the cancellation flag. This avoids double-removal race condition.

        // Update status immediately for UI responsiveness
        {
            let mut models = self.available_models.write().await;
            if let Some(model_info) = models.get_mut(model_name) {
                model_info.status = ModelStatus::NotDownloaded;
            }
        }

        // Brief delay to let download loop detect cancellation
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// Delete a corrupted or available model file
    pub async fn delete_model(&self, model_name: &str) -> Result<()> {
        log::info!("Deleting model: {}", model_name);

        let model_def = get_model_by_name_any(model_name)
            .ok_or_else(|| anyhow!("Unknown model: {}", model_name))?;

        let file_path = self
            .models_dir
            .join(model_def.subdir())
            .join(&model_def.gguf_file);

        if file_path.exists() {
            fs::remove_file(&file_path).await?;
            log::info!("Deleted model file: {}", file_path.display());
        }

        // Update status
        {
            let mut models = self.available_models.write().await;
            if let Some(model_info) = models.get_mut(model_name) {
                model_info.status = ModelStatus::NotDownloaded;
            }
        }

        Ok(())
    }

    /// Get models directory path
    pub fn get_models_directory(&self) -> PathBuf {
        self.models_dir.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::summary_engine::models::{
        add_to_custom_registry, refresh_custom_registry_cache, ModelType,
    };
    use std::io::Write;

    /// Regression test: a custom model added to the registry after the manager
    /// was initialized must still appear as Available after download_model_detailed
    /// discovers the file already exists.
    #[tokio::test]
    async fn custom_model_in_registry_with_existing_file_appears_available() {
        let temp_dir =
            std::env::temp_dir().join(format!("poly_test_model_manager_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let models_dir = temp_dir.join("models");
        std::fs::create_dir_all(&models_dir).unwrap();

        let manager = ModelManager::new_with_models_dir(models_dir.clone()).unwrap();
        manager.init().await.unwrap();

        let repo_id = "test-org/test-model";
        let filename = "model-q4_k_m.gguf";
        let size_bytes: u64 = 100_000_000;
        add_to_custom_registry(&temp_dir, repo_id, filename, "gemma3", 8192, size_bytes).unwrap();
        refresh_custom_registry_cache(&temp_dir);

        let file_path = models_dir.join("summary").join(filename);
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"GGUF").unwrap();
        let remaining = size_bytes - 4;
        file.write_all(&vec![0u8; remaining as usize]).unwrap();
        drop(file);

        let model_name = "custom:test-org:test-model";

        // Defend against concurrent-test cache invalidation: re-refresh
        // immediately before the download so the global CUSTOM_REGISTRY is
        // populated with entries from this test's temp dir.
        refresh_custom_registry_cache(&temp_dir);
        manager
            .download_model_detailed(model_name, None)
            .await
            .unwrap();

        let models = manager.list_models().await;
        let custom = models
            .iter()
            .find(|m| m.name == model_name)
            .expect("custom model should appear in list_models after download");

        assert_eq!(custom.status, ModelStatus::Available);
        assert_eq!(custom.model_type, ModelType::Summary);
        assert_eq!(custom.gguf_file, filename);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn custom_model_visible_after_startup_init() {
        let temp_dir = std::env::temp_dir().join(format!(
            "poly_test_model_manager_startup_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let summary_dir = temp_dir.join("models").join("summary");
        std::fs::create_dir_all(&summary_dir).unwrap();

        let repo_id = "test-org/startup-test-model";
        let filename = "model.gguf";
        let size_bytes: u64 = 100_000_000;
        add_to_custom_registry(&temp_dir, repo_id, filename, "gemma3", 8192, size_bytes).unwrap();
        refresh_custom_registry_cache(&temp_dir);

        let file_path = summary_dir.join(filename);
        let file = std::fs::File::create(&file_path).unwrap();
        file.set_len(size_bytes).unwrap();
        drop(file);

        // Defend against concurrent-test cache invalidation: re-refresh
        // immediately before init so the global CUSTOM_REGISTRY is
        // populated with entries from this test's temp dir.
        refresh_custom_registry_cache(&temp_dir);

        let manager = ModelManager::new_with_models_dir(temp_dir.join("models")).unwrap();
        manager.init().await.unwrap();

        // Refresh cache once more before querying. Parallel tests may have
        // overwritten the global CUSTOM_REGISTRY between our earlier refresh
        // and the scan inside init().
        refresh_custom_registry_cache(&temp_dir);

        let model_name = "custom:test-org:startup-test-model";
        let models = manager.list_models().await;
        let custom = models
            .iter()
            .find(|m| m.name == model_name)
            .expect("custom model should appear in list_models after init");

        assert_eq!(custom.status, ModelStatus::Available);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
