//! Background MCP HTTP server lifecycle management for the TUI event loop.

use super::app_event::AppEvent;
use regenerator2000_core::mcp::http;
use std::net::SocketAddr;
use std::sync::mpsc::Sender;
use std::thread;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::oneshot;

/// Spawns the MCP HTTP server thread on default port `3000` and returns a shutdown sender handle.
#[must_use]
pub fn spawn_mcp_server_thread(event_tx: &Sender<AppEvent>) -> Option<oneshot::Sender<()>> {
    spawn_mcp_server_thread_on_port(event_tx, 3000)
}

/// Spawns the MCP HTTP server thread on `port` and returns a shutdown sender handle.
///
/// The background thread binds `127.0.0.1:{port}` before it emits [`AppEvent::McpServerStarted`].
/// If binding fails, the thread emits [`AppEvent::McpError`] and exits without
/// [`AppEvent::McpServerStopped`]. This keeps the bind error visible in the TUI status bar.
#[must_use]
pub fn spawn_mcp_server_thread_on_port(
    event_tx: &Sender<AppEvent>,
    port: u16,
) -> Option<oneshot::Sender<()>> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let (mcp_req_tx, mut mcp_req_rx) = tokio_mpsc::channel(100);
    let mcp_bridge_tx = event_tx.clone();
    let started_tx = event_tx.clone();
    let stopped_tx = event_tx.clone();

    thread::spawn(move || {
        let Ok(rt) = Runtime::new() else {
            let _ = mcp_bridge_tx.send(AppEvent::McpError(
                "Failed to initialize Tokio runtime for MCP server".to_string(),
            ));
            return;
        };
        rt.block_on(async move {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = match TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    log::error!("Failed to bind MCP HTTP server on 127.0.0.1:{port}: {e}");
                    let _ = mcp_bridge_tx.send(AppEvent::McpError(format!(
                        "Failed to bind 127.0.0.1:{port}: {e}"
                    )));
                    return;
                }
            };

            let bound_port = listener.local_addr().map_or(port, |a| a.port());
            let server_handle = tokio::spawn(async move {
                http::serve_listener_with_shutdown(listener, mcp_req_tx, shutdown_rx).await
            });

            let _ = started_tx.send(AppEvent::McpServerStarted { port: bound_port });

            while let Some(req) = mcp_req_rx.recv().await {
                if mcp_bridge_tx.send(AppEvent::Mcp(req)).is_err() {
                    break;
                }
            }

            match server_handle.await {
                Ok(Err(e)) => {
                    log::error!("MCP HTTP server error on 127.0.0.1:{bound_port}: {e}");
                    let _ = stopped_tx.send(AppEvent::McpError(e.to_string()));
                }
                Ok(Ok(())) | Err(_) => {
                    let _ = stopped_tx.send(AppEvent::McpServerStopped);
                }
            }
        });
    });

    Some(shutdown_tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener as StdTcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn mcp_server_bind_failure_emits_only_mcp_error_without_clobbering() {
        // Hold an ephemeral port so `spawn_mcp_server_thread_on_port` fails with `Address already in use`.
        let occupied = StdTcpListener::bind("127.0.0.1:0").expect("bind ephemeral port for test");
        let port = occupied.local_addr().expect("local_addr").port();

        let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
        let _shutdown_tx = spawn_mcp_server_thread_on_port(&event_tx, port);
        drop(event_tx);

        let mut events = Vec::new();
        while let Ok(ev) = event_rx.recv_timeout(Duration::from_secs(5)) {
            events.push(ev);
        }

        assert_eq!(
            events.len(),
            1,
            "Expected exactly 1 event (McpError) on port bind conflict, got {}",
            events.len()
        );
        match &events[0] {
            AppEvent::McpError(msg) => {
                assert!(
                    msg.contains(&format!("127.0.0.1:{port}")),
                    "Expected error message to mention port {port}, got: {msg}"
                );
            }
            _ => panic!("Expected AppEvent::McpError on port bind conflict"),
        }
    }

    #[test]
    fn mcp_server_clean_start_and_stop_lifecycle() {
        let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
        let shutdown_tx =
            spawn_mcp_server_thread_on_port(&event_tx, 0).expect("shutdown_tx returned");
        drop(event_tx);

        let first = event_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Expected McpServerStarted event");
        match first {
            AppEvent::McpServerStarted { port } => {
                assert!(port > 0, "Expected non-zero bound ephemeral port");
            }
            _ => panic!("Expected AppEvent::McpServerStarted as first event"),
        }

        shutdown_tx.send(()).expect("send shutdown signal");

        let second = event_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Expected McpServerStopped event");
        assert!(
            matches!(second, AppEvent::McpServerStopped),
            "Expected AppEvent::McpServerStopped after shutdown"
        );
    }
}
