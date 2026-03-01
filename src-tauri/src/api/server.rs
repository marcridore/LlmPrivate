use std::sync::Arc;

use axum::{routing::get, Router};
use tokio::net::TcpListener;

use crate::api::routes;
use crate::state::AppState;

pub struct ApiServer {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    port: u16,
}

impl ApiServer {
    pub fn new() -> Self {
        Self {
            shutdown_tx: None,
            port: 8080,
        }
    }

    pub async fn start(
        &mut self,
        _app_state: Arc<AppState>,
        port: u16,
    ) -> Result<(), anyhow::Error> {
        self.port = port;

        let router = Router::new()
            .route("/health", get(routes::health))
            .layer(tower_http::cors::CorsLayer::permissive());

        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        self.shutdown_tx = Some(tx);

        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    rx.await.ok();
                })
                .await
                .ok();
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(()).ok();
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }
}
