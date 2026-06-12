//! Graph HTTP server on `127.0.0.1:9847`.

use graph_server::{AppState, DEFAULT_ADDR, router};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "graph_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = AppState::new();
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(DEFAULT_ADDR)
        .await
        .expect("bind graph-server");

    tracing::info!("graph-server listening on http://{DEFAULT_ADDR}");
    axum::serve(listener, app)
        .await
        .expect("graph-server failed");
}
