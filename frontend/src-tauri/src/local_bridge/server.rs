use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tiny_http::{Header, Method, Response};

use crate::local_bridge::embedding::EmbeddingEngine;
use crate::local_bridge::models;
use crate::summary::summary_engine;

pub struct BridgeServer {
    app_data_dir: PathBuf,
    port: u16,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl BridgeServer {
    pub fn new(app_data_dir: PathBuf, port: u16) -> Self {
        Self {
            app_data_dir,
            port,
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(anyhow::anyhow!("Bridge server is already running"));
        }

        let addr = format!("127.0.0.1:{}", self.port);
        let server =
            tiny_http::Server::http(&addr).map_err(|e| anyhow::anyhow!("Failed to bind: {}", e))?;

        log::info!("Local bridge started on {}", addr);

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let app_data_dir = self.app_data_dir.clone();

        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_secs(1)) {
                    Ok(Some(request)) => {
                        // Catch panics so a single bad request doesn't kill the server.
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            handle_request(request, &app_data_dir);
                        }));
                        if let Err(e) = result {
                            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = e.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "unknown panic".to_string()
                            };
                            log::error!("Bridge handler panicked: {}", msg);
                        }
                    }
                    Ok(None) => {
                        continue;
                    }
                    Err(e) => {
                        if running.load(Ordering::SeqCst) {
                            log::error!("Bridge server error: {}", e);
                        }
                        break;
                    }
                }
            }

            log::info!("Local bridge server shut down");
        });

        self.handle = Some(handle);
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

fn handle_request(request: tiny_http::Request, app_data_dir: &PathBuf) {
    let url = request.url().to_string();
    let method = request.method();

    log::trace!("Bridge: {:?} {}", method, url);

    match (method, url.as_str()) {
        (&Method::Get, "/health") => respond_health(request),
        (&Method::Get, "/v1/models") => respond_models(request),
        (&Method::Post, "/v1/chat/completions") => respond_chat_completions(request, app_data_dir),
        (&Method::Post, "/v1/embeddings") => respond_embeddings(request),
        _ => {
            let error_body = serde_json::to_string(&models::ErrorResponse::new(
                "Not found".to_string(),
                "not_found".to_string(),
            ))
            .unwrap();
            let response = Response::from_string(error_body)
                .with_status_code(404)
                .with_header(json_content_type());
            let _ = request.respond(response);
        }
    }
}

fn json_content_type() -> Header {
    "Content-Type: application/json".parse::<Header>().unwrap()
}

fn respond_health(request: tiny_http::Request) {
    let body = serde_json::json!({"status": "ok"});
    let response = Response::from_string(body.to_string())
        .with_status_code(200)
        .with_header(json_content_type());
    let _ = request.respond(response);
}

fn respond_models(request: tiny_http::Request) {
    let available = summary_engine::models::get_available_models();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let data: Vec<models::ModelObject> = available
        .iter()
        .map(|m| models::ModelObject {
            id: m.name.clone(),
            object: "model".to_string(),
            created: now,
            owned_by: "poly".to_string(),
        })
        .collect();

    let model_list = models::ModelList {
        object: "list".to_string(),
        data,
    };

    let body = serde_json::to_string(&model_list).unwrap();
    let response = Response::from_string(body)
        .with_status_code(200)
        .with_header(json_content_type());
    let _ = request.respond(response);
}

fn respond_chat_completions(mut request: tiny_http::Request, app_data_dir: &PathBuf) {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        send_error(
            request,
            400,
            "Failed to read request body",
            "invalid_request",
        );
        return;
    }

    let chat_request: models::ChatCompletionRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            send_error(
                request,
                400,
                &format!("Invalid request body: {}", e),
                "invalid_request",
            );
            return;
        }
    };

    if chat_request.stream.unwrap_or(false) {
        send_error(
            request,
            400,
            "Streaming is not supported by the local bridge",
            "invalid_request",
        );
        return;
    }

    let system_prompt: String = chat_request
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n");

    let user_prompt: String = chat_request
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = if system_prompt.is_empty() {
        "You are a helpful assistant.".to_string()
    } else {
        system_prompt
    };

    log::info!(
        "Bridge chat completion: model={}, system_len={}, user_len={}",
        chat_request.model,
        system_prompt.len(),
        user_prompt.len()
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

    match rt {
        Ok(rt) => {
            let result = rt.block_on(summary_engine::client::generate_with_builtin(
                app_data_dir,
                &chat_request.model,
                &system_prompt,
                &user_prompt,
                None,
            ));

            match result {
                Ok(content) => {
                    respond_success(request, &chat_request.model, content);
                }
                Err(e) => {
                    send_error(
                        request,
                        500,
                        &format!("Generation failed: {}", e),
                        "generation_error",
                    );
                }
            }
        }
        Err(e) => {
            send_error(
                request,
                500,
                &format!("Internal error: {}", e),
                "internal_error",
            );
        }
    }
}

fn respond_success(request: tiny_http::Request, model: &str, content: String) {
    let now = chrono::Utc::now().timestamp();

    let completion = models::ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: now,
        model: model.to_string(),
        choices: vec![models::Choice {
            index: 0,
            message: models::ChoiceMessage {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    };

    let body = serde_json::to_string(&completion).unwrap();
    let response = Response::from_string(body)
        .with_status_code(200)
        .with_header(json_content_type());
    let _ = request.respond(response);
}

fn respond_embeddings(mut request: tiny_http::Request) {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        send_error(
            request,
            400,
            "Failed to read request body",
            "invalid_request",
        );
        return;
    }

    let emb_request: models::EmbeddingRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            send_error(
                request,
                400,
                &format!("Invalid request body: {}", e),
                "invalid_request",
            );
            return;
        }
    };

    let engine = EmbeddingEngine::new(&emb_request.model);
    let input_texts = emb_request.input.as_vec();
    let dim = emb_request.dimensions.unwrap_or(engine.dimensions());

    let engine = if dim != engine.dimensions() {
        EmbeddingEngine::with_dimension(&emb_request.model, dim)
    } else {
        engine
    };

    let vectors = engine.embed(&input_texts);
    let prompt_tokens: i32 = input_texts.iter().map(|t| estimate_tokens(t)).sum();

    let data: Vec<models::EmbeddingObject> = vectors
        .into_iter()
        .enumerate()
        .map(|(i, embedding)| models::EmbeddingObject {
            object: "embedding".to_string(),
            index: i,
            embedding,
        })
        .collect();

    let response = models::EmbeddingResponse {
        object: "list".to_string(),
        data,
        model: emb_request.model,
        usage: models::EmbeddingUsage {
            prompt_tokens,
            total_tokens: prompt_tokens,
        },
    };

    let body = serde_json::to_string(&response).unwrap();
    let response = Response::from_string(body)
        .with_status_code(200)
        .with_header(json_content_type());
    let _ = request.respond(response);
}

/// Rough token estimator: ~4 chars per token for English text.
fn estimate_tokens(text: &str) -> i32 {
    let len = text.chars().count();
    if len == 0 {
        return 0;
    }
    (len.max(3) / 4) as i32
}

fn send_error(request: tiny_http::Request, status: u16, message: &str, error_type: &str) {
    let error_body = serde_json::to_string(&models::ErrorResponse::new(
        message.to_string(),
        error_type.to_string(),
    ))
    .unwrap();
    let response = Response::from_string(error_body)
        .with_status_code(status)
        .with_header(json_content_type());
    let _ = request.respond(response);
}
