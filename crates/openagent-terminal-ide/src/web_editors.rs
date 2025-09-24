//! Web editor functionality for OpenAgent Terminal IDE

use crate::IdeResult;

#[cfg(feature = "web-editors")]
use axum::{routing::get, Router};
#[cfg(feature = "web-editors")]
use std::net::SocketAddr;
#[cfg(feature = "web-editors")]
use tokio::sync::oneshot;

/// Web editor manager with HTTP server and GTK4 webview integration.
#[derive(Debug, Default)]
pub struct WebEditorManager {
    #[cfg(feature = "web-editors")]
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl WebEditorManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a minimal, production-ready HTTP server for the web editor.
    /// Requires a Tokio runtime in the caller.
    #[cfg(feature = "web-editors")]
    pub fn start_server(&mut self, port: u16) -> IdeResult<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let app = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route(
                "/",
                get(|| async {
                    axum::response::Html(
                        "<html><head><meta charset=\"utf-8\"></head><body><h1>OpenAgent IDE</h1><p>Web editor is running.</p></body></html>",
                    )
                }),
            );

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        tokio::spawn(async move {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    tracing::info!(addr = %addr, "web-editor.server_started");
                    let server = axum::serve(listener, app.into_make_service())
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_rx.await;
                        });
                    if let Err(err) = server.await {
                        tracing::error!(error = %err, "web-editor.server_error");
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "web-editor.bind_failed");
                }
            }
        });

        Ok(())
    }

    /// Request graceful shutdown of the web editor server if running.
    #[cfg(feature = "web-editors")]
    pub fn stop_server(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Launch a GTK4-backed webview window and block until it is closed.
    /// Should be called from the process main thread on Linux.
    #[cfg(feature = "web-editors")]
    pub fn launch_webview_blocking(&self, url: &str, title: &str, width: u32, height: u32) -> IdeResult<()> {
        super::gtk4_ui::open_webview_blocking(url, title, width, height)
    }
}
