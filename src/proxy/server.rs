use super::handlers::*;
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// DNS-Rebinding & Host Header Guard for loopback HTTP server (`127.0.0.1:3030`).
async fn loopback_host_guard(req: Request, next: Next) -> Result<Response, StatusCode> {
    if let Some(host_val) = req.headers().get(header::HOST) {
        let h = host_val.to_str().unwrap_or("");
        let valid = h == "localhost"
            || h == "127.0.0.1"
            || h == "[::1]"
            || h.starts_with("localhost:")
            || h.starts_with("127.0.0.1:")
            || h.starts_with("[::1]:");
        if !valid {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(req).await)
}

pub struct ApiServer {
    state: Arc<AppState>,
    port: u16,
}

impl ApiServer {
    pub fn new(state: Arc<AppState>, port: u16) -> Self {
        Self { state, port }
    }

    pub fn router(&self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::predicate(|origin, _| {
                if let Ok(s) = origin.to_str() {
                    s.starts_with("http://localhost:")
                        || s.starts_with("http://127.0.0.1:")
                        || s.starts_with("http://[::1]:")
                } else {
                    false
                }
            }))
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/api/health", get(health_check))
            .route("/api/models", get(list_models))
            .route("/api/temporal/check", post(check_temporal))
            .route("/api/research/search", post(search_sources))
            .route("/api/grounding/verify", post(verify_grounding))
            .route("/api/memory/entity", post(save_entity))
            .route("/api/memory/graph/:project_id", get(get_memory_graph))
            .route("/api/memory/dossier/:project_id", get(get_dossier))
            .route("/api/chat/epistemic", post(epistemic_chat))
            .route("/api/drift/events", get(get_drift_events))
            .route("/api/security/audit", get(get_security_audit))
            .route("/api/cost/metrics", get(get_cost_metrics))
            .route("/api/cost/route", post(route_tools))
            .route("/api/gateway/servers", get(list_gateway_servers).post(register_gateway_server))
            .route("/api/gateway/tools", get(list_gateway_tools))
            .route("/api/gateway/call", post(call_gateway_tool))
            .layer(middleware::from_fn(loopback_host_guard))
            .layer(cors)
            .with_state(self.state.clone())
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.router();
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("🚀 ChronoFact Epistemic API Server listening on http://{}", addr);
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        tracing::info!("🛑 ChronoFact Epistemic API Server gracefully stopped.");
        Ok(())
    }

    /// Co-hosts the ChronoFact HTTP API on `127.0.0.1:3030` in a background task when launched in `mcp` mode.
    /// If port `3030` is already bound by an existing daemon, silently skips binding.
    pub async fn try_run_background(self) {
        let app = self.router();
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
            tracing::info!("🚀 ChronoFact Co-Hosted HTTP API active on http://{}", addr);
            let _ = axum::serve(listener, app).await;
        } else {
            tracing::info!("ChronoFact HTTP API port {} already bound by an active daemon.", self.port);
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
