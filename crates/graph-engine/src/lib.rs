//! Query layer over indexed code graphs.

mod engine;
mod overlay;

pub use engine::{
    BoundaryDirection, GhostNode, GraphEngine, Neighbor, Subgraph, TraceHop, TraceResult,
};
pub use overlay::{
    attach_overlay, otel_json_to_overlay, parse_runtime_overlay, ObservedPath, OverlayParseError,
    RuntimeOverlay, SubgraphOverlay,
};
pub use graph_indexer::{GraphEdge, GraphNode, NodeKind};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub enum EngineError {
    Indexer(graph_indexer::IndexError),
    NotFound(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Indexer(e) => write!(f, "{e}"),
            EngineError::NotFound(id) => write!(f, "node not found: {id}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<graph_indexer::IndexError> for EngineError {
    fn from(value: graph_indexer::IndexError) -> Self {
        EngineError::Indexer(value)
    }
}
