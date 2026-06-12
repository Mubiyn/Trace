//! MCP stdio server — indexes a repo then serves graph tools.

use graph_server::{AppState, run_mcp_stdio};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let repo = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: graph-mcp <repo-path>");
            std::process::exit(1);
        });

    let state = AppState::new();
    if let Err(err) = run_mcp_stdio(state, &repo).await {
        eprintln!("graph-mcp error: {err}");
        std::process::exit(1);
    }
}
