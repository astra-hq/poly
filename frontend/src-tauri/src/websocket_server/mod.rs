pub mod broadcaster;
pub mod server;

use std::sync::Arc;

use tokio::sync::watch;

pub use broadcaster::initialize_broadcaster;

pub struct WebSocketServerHandle {
    shutdown_tx: watch::Sender<bool>,
}

impl WebSocketServerHandle {
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

pub fn start_websocket_server(bind_address: String, port: u16) -> WebSocketServerHandle {
    initialize_broadcaster();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        if let Err(e) = server::start_server(bind_address, port, shutdown_rx).await {
            log::error!("WebSocket server error: {}", e);
        }
    });

    WebSocketServerHandle { shutdown_tx }
}
