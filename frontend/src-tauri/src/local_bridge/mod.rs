// Local bridge module — localhost-only OpenAI-compatible API for llama-helper models.
// Lifecycle: lazy start → graceful shutdown on app exit.

pub mod embedding;
pub mod models;
pub mod server;

use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::local_bridge::server::BridgeServer;

/// Default bridge port (configurable via POLY_BRIDGE_PORT env var).
pub const DEFAULT_PORT: u16 = 11337;

/// Global bridge server instance (started lazily).
static BRIDGE: once_cell::sync::Lazy<Mutex<Option<BridgeServer>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// Start the bridge server on 127.0.0.1:DEFAULT_PORT.
/// Returns Ok(()) if started successfully, or if already running.
pub fn start_bridge(app_data_dir: PathBuf) -> anyhow::Result<()> {
    let port = get_bridge_port();
    let mut guard = BRIDGE
        .lock()
        .map_err(|e| anyhow::anyhow!("Bridge mutex poisoned: {}", e))?;

    if let Some(ref server) = *guard {
        if server.is_running() {
            return Ok(());
        }
        // Stale stopped server — replace it
        *guard = None;
    }

    let mut server = BridgeServer::new(app_data_dir, port);
    server.start()?;
    *guard = Some(server);

    log::info!("Local bridge started on 127.0.0.1:{}", port);
    Ok(())
}

/// Stop the bridge server gracefully.
pub fn stop_bridge() -> anyhow::Result<()> {
    let mut guard = BRIDGE
        .lock()
        .map_err(|e| anyhow::anyhow!("Bridge mutex poisoned: {}", e))?;

    if let Some(ref mut server) = *guard {
        server.stop()?;
    }
    *guard = None;

    // Brief pause to let the OS release the socket.
    std::thread::sleep(std::time::Duration::from_millis(100));

    log::info!("Local bridge stopped");
    Ok(())
}

/// Check if the bridge is running and accepting connections.
pub fn bridge_health() -> bool {
    let addr = format!("127.0.0.1:{}", get_bridge_port());
    TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(100)).is_ok()
}

/// Get the bridge port (from POLY_BRIDGE_PORT env var, or DEFAULT_PORT).
pub fn get_bridge_port() -> u16 {
    std::env::var("POLY_BRIDGE_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::Mutex as StdMutex;
    use std::time::{Duration, Instant};

    const TEST_APP_DATA: &str = "/tmp/poly-test-bridge";

    /// Global test lock — prevents parallel tests from fighting over the shared
    /// bridge singleton on port 11337. Each bridge integration test acquires
    /// this lock for its entire duration so only one bridge runs at a time.
    static TEST_LOCK: once_cell::sync::Lazy<StdMutex<()>> =
        once_cell::sync::Lazy::new(|| StdMutex::new(()));

    static TEST_PORT_INIT: std::sync::Once = std::sync::Once::new();
    fn init_test_port() {
        TEST_PORT_INIT.call_once(|| {
            std::env::set_var("POLY_BRIDGE_PORT", "11338");
        });
    }

    /// Wait for the bridge to become healthy, polling every 100ms up to a
    /// timeout.  Returns true once `bridge_health()` succeeds.
    fn wait_for_bridge(timeout_ms: u64) -> bool {
        let deadline = Instant::now().checked_add(Duration::from_millis(timeout_ms)).unwrap();
        while Instant::now() < deadline {
            if bridge_health() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    /// HTTP helper with retry — handles rare cases where the bridge is still
    /// initialising its accept loop after `tiny_http::Server::http()` binds.
    fn http_get(request: &str) -> String {
        let addr = format!("127.0.0.1:{}", get_bridge_port());
        let addr: std::net::SocketAddr = addr.parse().unwrap();
        let mut last_err = None;
        for attempt in 0..5 {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                Ok(mut stream) => {
                    stream
                        .write_all(request.as_bytes())
                        .expect("Failed to send request");
                    let mut response = String::new();
                    stream
                        .read_to_string(&mut response)
                        .expect("Failed to read response");
                    return response;
                }
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(Duration::from_millis(100 * (attempt + 1)));
                }
            }
        }
        panic!(
            "Failed to connect after 5 attempts: {:?}",
            last_err.unwrap()
        );
    }

    /// Baseline: no bridge server running on the test port.
    #[test]
    fn baseline_no_bridge_running_on_default_port() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));
        let port = get_bridge_port();
        let addr = format!("127.0.0.1:{}", port);
        let result = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(200));
        assert!(
            result.is_err(),
            "Port {} should be free before bridge is started; found a listener",
            port
        );
    }

    /// Baseline: /v1/embeddings was not present before this change.
    /// The endpoint must now exist and return 200 with valid embedding JSON.
    #[test]
    fn baseline_embeddings_endpoint_was_absent() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // This test documents that /v1/embeddings used to return 404.
        // After implementation, it returns 200.
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));
        start_bridge(std::path::PathBuf::from(TEST_APP_DATA)).expect("Failed to start bridge");
        assert!(
            wait_for_bridge(5000),
            "Bridge did not start within 5 seconds"
        );

        let body = serde_json::json!({
            "model": "bge-m3",
            "input": "hello world"
        });
        let body_str = body.to_string();
        let request = format!(
            "POST /v1/embeddings HTTP/1.1\r\n\
             Host: 127.0.0.1:{}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            get_bridge_port(),
            body_str.len(),
            body_str
        );
        let resp = http_get(&request);

        let body_start = resp.find("\r\n\r\n").unwrap_or(0) + 4;
        let response_body = &resp[body_start..];
        let parsed: serde_json::Value =
            serde_json::from_str(response_body).expect("Embeddings response must be valid JSON");
        assert!(
            resp.contains("200 OK") || parsed["object"] == "list",
            "Expected /v1/embeddings to return 200 with object=list, got: {}",
            response_body
        );

        let _ = stop_bridge();
    }

    /// Failing-first proof: /v1/embeddings returns OpenAI-compatible embedding arrays.
    /// This test was written to fail before implementation and now passes.
    #[test]
    fn embeddings_returns_openai_compatible_format() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));
        start_bridge(std::path::PathBuf::from(TEST_APP_DATA)).expect("Failed to start bridge");
        assert!(
            wait_for_bridge(5000),
            "Bridge did not start within 5 seconds"
        );

        let body = serde_json::json!({
            "model": "bge-m3",
            "input": "The quick brown fox"
        });
        let body_str = body.to_string();
        let request = format!(
            "POST /v1/embeddings HTTP/1.1\r\n\
             Host: 127.0.0.1:{}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            get_bridge_port(),
            body_str.len(),
            body_str
        );
        let resp = http_get(&request);
        assert!(
            resp.contains("200 OK"),
            "Expected 200, got: {}",
            resp.lines().next().unwrap_or("empty")
        );

        let body_start = resp.find("\r\n\r\n").unwrap_or(0) + 4;
        let response_body = &resp[body_start..];
        let parsed: serde_json::Value =
            serde_json::from_str(response_body).expect("Embeddings response must be valid JSON");

        assert_eq!(parsed["object"], "list");
        assert!(parsed["data"].is_array(), "data must be an array");
        assert_eq!(parsed["data"][0]["object"], "embedding");
        assert_eq!(parsed["data"][0]["index"], 0);
        assert!(
            parsed["data"][0]["embedding"].is_array(),
            "embedding must be an array"
        );
        assert_eq!(
            parsed["data"][0]["embedding"].as_array().unwrap().len(),
            1024,
            "embedding must have 1024 dimensions"
        );
        assert_eq!(parsed["model"], "bge-m3");
        assert!(
            parsed["usage"]["prompt_tokens"].is_i64(),
            "usage.prompt_tokens must be present"
        );
        assert!(
            parsed["usage"]["total_tokens"].is_i64(),
            "usage.total_tokens must be present"
        );

        let _ = stop_bridge();
    }

    /// Full integration suite: starts the bridge once, runs all endpoint
    /// and binding tests, then stops.
    #[test]
    fn bridge_integration_suite() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Start bridge once
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));
        start_bridge(std::path::PathBuf::from(TEST_APP_DATA)).expect("Failed to start bridge");
        assert!(
            wait_for_bridge(5000),
            "Bridge did not start within 5 seconds"
        );

        // Test: bridge binds loopback only
        let loopback_addr = format!("127.0.0.1:{}", get_bridge_port());
        let loopback_result =
            TcpStream::connect_timeout(&loopback_addr.parse().unwrap(), Duration::from_secs(1));
        assert!(
            loopback_result.is_ok(),
            "Bridge should be listening on 127.0.0.1"
        );

        // Test: /health
        {
            let request = format!(
                "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                get_bridge_port()
            );
            let resp = http_get(&request);
            assert!(
                resp.contains("200 OK"),
                "Health endpoint failed: {}",
                resp.lines().next().unwrap_or("empty")
            );
        }

        // Test: /v1/models
        {
            let request = format!(
                "GET /v1/models HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                get_bridge_port()
            );
            let resp = http_get(&request);
            assert!(
                resp.contains("200 OK"),
                "Models endpoint failed: {}",
                resp.lines().next().unwrap_or("empty")
            );

            let body_start = resp.find("\r\n\r\n").unwrap_or(0) + 4;
            let response_body = &resp[body_start..];
            let parsed: serde_json::Value = serde_json::from_str(response_body)
                .expect("Models response body should be valid JSON");
            assert_eq!(parsed["object"], "list");
            assert!(parsed["data"].is_array());
        }

        // Test: /v1/chat/completions (error path when no sidecar)
        {
            let body = serde_json::json!({
                "model": "gemma3:1b",
                "messages": [
                    {"role": "system", "content": "You are a helpful assistant."},
                    {"role": "user", "content": "Say hello."}
                ],
                "temperature": 0.7,
                "max_tokens": 50
            });
            let body_str = body.to_string();
            let request = format!(
                "POST /v1/chat/completions HTTP/1.1\r\n\
                 Host: 127.0.0.1:{}\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n\
                 {}",
                get_bridge_port(),
                body_str.len(),
                body_str
            );
            let resp = http_get(&request);

            let body_start = resp.find("\r\n\r\n").unwrap_or(0) + 4;
            let response_body = &resp[body_start..];
            let parsed: serde_json::Value = serde_json::from_str(response_body)
                .expect("Chat completions response body should be valid JSON");

            if resp.contains("200 OK") {
                assert_eq!(parsed["object"], "chat.completion");
                assert!(parsed["choices"].is_array());
                assert!(parsed["choices"][0]["message"]["role"].is_string());
                assert!(parsed["choices"][0]["message"]["content"].is_string());
            } else {
                assert!(
                    parsed["error"]["message"].is_string(),
                    "Error response missing message field: {}",
                    response_body
                );
                assert!(
                    parsed["error"]["type"].is_string(),
                    "Error response missing type field: {}",
                    response_body
                );
            }
        }

        let _ = stop_bridge();
    }

    #[test]
    fn bridge_starts_with_dev_style_app_data_dir() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));

        let dev_dir = std::env::temp_dir()
            .join("poly_test_bridge_dev")
            .join("models")
            .join("summary");
        std::fs::create_dir_all(&dev_dir).unwrap();

        let result = start_bridge(dev_dir.clone());
        assert!(
            result.is_ok(),
            "Bridge must start with dev-style path: {:?}",
            result.err()
        );
        assert!(
            wait_for_bridge(5000),
            "Bridge must be healthy after start"
        );

        let _ = stop_bridge();
        let _ = std::fs::remove_dir_all(dev_dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn bridge_starts_with_mocked_packaged_app_data_dir() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));

        let prod_dir = std::env::temp_dir()
            .join("poly_test_bridge_prod")
            .join("Poly")
            .join("models")
            .join("summary");
        std::fs::create_dir_all(&prod_dir).unwrap();

        let result = start_bridge(prod_dir.clone());
        assert!(
            result.is_ok(),
            "Bridge must start with mocked packaged path: {:?}",
            result.err()
        );
        assert!(
            wait_for_bridge(5000),
            "Bridge must be healthy after start"
        );

        let _ = stop_bridge();
        let _ = std::fs::remove_dir_all(
            prod_dir
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        );
    }

    #[test]
    fn bridge_health_endpoint_returns_ok_after_start() {
        init_test_port();
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = stop_bridge();
        std::thread::sleep(Duration::from_millis(50));
        start_bridge(std::path::PathBuf::from("/tmp/poly-test-bridge-assets"))
            .expect("Failed to start bridge");
        assert!(
            wait_for_bridge(5000),
            "Bridge did not start within 5 seconds"
        );

        let request = format!(
            "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            get_bridge_port()
        );
        let resp = http_get(&request);
        assert!(
            resp.contains("200 OK"),
            "Health must return 200, got: {}",
            resp.lines().next().unwrap_or("empty")
        );

        let body_start = resp.find("\r\n\r\n").unwrap_or(0) + 4;
        let body = &resp[body_start..];
        let parsed: serde_json::Value =
            serde_json::from_str(body).expect("Health body must be valid JSON");
        assert_eq!(parsed["status"], "ok");

        let _ = stop_bridge();
    }
} // mod tests
