use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use log;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::accept_async;

use super::broadcaster::{get_broadcaster, WsMessage};

pub async fn start_server(
    bind_address: String,
    port: u16,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", bind_address, port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    log::info!("WebSocket server listening on ws://{}", addr);

    loop {
        tokio::select! {
            Ok((stream, peer)) = listener.accept() => {
                let peer_addr = peer.to_string();
                log::info!("WebSocket client connected: {}", peer_addr);

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer_addr.clone()).await {
                        log::warn!("WebSocket connection error for {}: {}", peer_addr, e);
                    }
                });
            }
            _ = shutdown.changed() => {
                log::info!("WebSocket server shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    let broadcaster = match get_broadcaster() {
        Some(b) => b,
        None => {
            log::warn!("Broadcaster not initialized, closing connection");
            return Ok(());
        }
    };

    let mut rx = broadcaster.subscribe();

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                let json = serde_json::to_string(&msg)?;
                if let Err(e) = ws_tx.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await {
                    log::warn!("WebSocket send error for {}: {}", peer_addr, e);
                    break;
                }
            }
            Some(msg_result) = ws_rx.next() => {
                match msg_result {
                    Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                        log::info!("WebSocket client disconnected: {}", peer_addr);
                        break;
                    }
                    Ok(_) => {
                    }
                    Err(e) => {
                        log::warn!("WebSocket receive error for {}: {}", peer_addr, e);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
