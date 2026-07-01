//! Local HTTP ingress for browser and other extensions (127.0.0.1 only).

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chronicle_core::CanonicalEvent;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};

#[derive(Clone)]
struct IngressState {
    event_tx: mpsc::Sender<CanonicalEvent>,
}

pub async fn run(port: u16, event_tx: mpsc::Sender<CanonicalEvent>) {
    let state = IngressState { event_tx };
    let app = Router::new()
        .route("/v1/events", post(ingest_event))
        .route("/health", post(health).get(health))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            warn!("HTTP ingress bind failed on {addr}: {e}");
            return;
        }
    };

    info!("HTTP ingress listening on http://{addr}/v1/events");
    if let Err(e) = axum::serve(listener, app).await {
        warn!("HTTP ingress stopped: {e}");
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn ingest_event(
    State(state): State<IngressState>,
    Json(event): Json<CanonicalEvent>,
) -> StatusCode {
    match state.event_tx.send(event).await {
        Ok(()) => StatusCode::ACCEPTED,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
