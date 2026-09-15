mod analysis;
mod api;
mod domain;
mod error;
mod helpers;

use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use api::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hwledger_bench_cockpit=info,tower_http=info".into()),
        )
        .init();

    let data_path = std::env::var("BENCH_DATA_PATH")
        .or_else(|_| std::env::var("DATA_PATH"))
        .unwrap_or_default();
    if data_path.is_empty() {
        tracing::warn!("BENCH_DATA_PATH / DATA_PATH not set — data endpoints will fail");
    }

    let extra_raw = std::env::var("BENCH_EXTRA_DATA").unwrap_or_default();
    let extra_paths: Vec<String> = extra_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let dist_dir = std::env::var("BENCH_DIST_DIR")
        .unwrap_or_else(|_| "../dist".into());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8090);

    tracing::info!(dist_dir = %dist_dir, data_path = %data_path, port, "starting bench-cockpit");

    let state = Arc::new(AppState::new(data_path, extra_paths, dist_dir.clone()));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/health", get(api::health))
        .route("/state", get(api::api_state))
        .route("/data", get(api::api_state))
        .route("/history", get(api::api_history))
        .route("/export", get(api::api_export))
        .route(
            "/cells/{suite}/{task_id}/{variant}/raw",
            get(api::api_cell_raw),
        )
        .route(
            "/langfuse/status",
            get(api::langfuse::langfuse_status),
        )
        .route(
            "/langfuse/setup",
            post(api::langfuse::langfuse_setup),
        )
        .route(
            "/langfuse/traces",
            get(api::langfuse::langfuse_traces),
        )
        .route(
            "/langfuse/evaluators",
            post(api::langfuse::langfuse_evaluators),
        )
        .route("/eval/run", post(api::api_eval_run))
        .route("/eval/runs/{id}", get(api::api_eval_run_status))
        .with_state(state.clone());

    let ws_routes = Router::new()
        .route("/ws", get(api::ws_handler))
        .with_state(state.clone());

    let dist_path = std::path::PathBuf::from(&dist_dir);
    let spa_service =
        tower_http::services::ServeDir::new(&dist_path).append_index_html_on_directories(true);

    let app = Router::new()
        .nest("/api", api_routes)
        .merge(ws_routes)
        .route("/api/ws", get(api::ws_handler).with_state(state.clone()))
        .fallback_service(spa_service)
        .layer(cors)
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(tower_http::trace::DefaultMakeSpan::new().include_headers(false))
                .on_response(tower_http::trace::DefaultOnResponse::new()),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    println!("Dashboard -> http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
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

    tracing::info!("shutdown signal received, starting graceful shutdown");
}
