use agileplus_dashboard::app_state::DashboardStore;
use agileplus_dashboard::routes::router;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Initialize tracing/logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Build the dashboard store with seed (kitty-specs -> features -> kanban)
    let mut store = DashboardStore::seeded();

    // Try to wire the live governance client (governance audit / rubric scoring)
    store = match agileplus_governance::GovernanceClient::with_defaults().await {
        Ok(client) => store.with_governance(client),
        Err(e) => {
            tracing::warn!(error = %e, "GovernanceClient::with_defaults failed; /api/dashboard/governance/status will return not_initialized");
            store
        }
    };

    // Try to wire the live plane.so sync client (reads projects, issues, cycles)
    // PlaneClient::new(base_url, api_key, workspace_slug, project_id)
    let plane_args = (
        std::env::var("PLANE_BASE_URL").unwrap_or_else(|_| "https://api.plane.so".to_string()),
        std::env::var("PLANE_API_KEY").ok(),
        std::env::var("PLANE_WORKSPACE").ok(),
        std::env::var("PLANE_PROJECT").ok(),
    );
    store = match plane_args {
        (_, Some(api_key), Some(workspace), Some(project)) => {
            store.with_plane(agileplus_plane::PlaneClient::new(
                "https://api.plane.so".to_string(),
                api_key,
                workspace,
                project,
            ))
        }
        _ => {
            tracing::info!(
                "PLANE_API_KEY/PLANE_WORKSPACE/PLANE_PROJECT not set; /api/dashboard/plane/sync will return not_initialized"
            );
            store
        }
    };

    let state = Arc::new(tokio::sync::RwLock::new(store));
    let app = router(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8770);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "agileplus-dashboard listening");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
