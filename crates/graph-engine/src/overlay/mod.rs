//! Runtime observations merged onto a static subgraph.

pub mod otel;

use crate::Subgraph;
use graph_indexer::{GraphEdge, GraphStore};
use otel::OtelImportError;

pub use otel::otel_json_to_overlay;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum OverlayParseError {
    Otel(OtelImportError),
    Json(serde_json::Error),
}

impl std::fmt::Display for OverlayParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayParseError::Otel(e) => write!(f, "{e}"),
            OverlayParseError::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OverlayParseError {}

/// Parse native overlay JSON or OpenTelemetry trace export.
pub fn parse_runtime_overlay(
    value: &Value,
    store: &GraphStore,
) -> Result<RuntimeOverlay, OverlayParseError> {
    if is_otel_payload(value) {
        return otel_json_to_overlay(value, store).map_err(OverlayParseError::Otel);
    }
    serde_json::from_value(value.clone()).map_err(OverlayParseError::Json)
}

fn is_otel_payload(value: &Value) -> bool {
    value.get("resourceSpans").is_some()
        || value
            .get("spans")
            .and_then(Value::as_array)
            .is_some_and(|s| !s.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverlay {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub node_hits: HashMap<String, u64>,
    #[serde(default)]
    pub paths: Vec<ObservedPath>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObservedPath {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubgraphOverlay {
    pub node_hits: HashMap<String, u64>,
    pub paths: Vec<ObservedPath>,
    pub observed_node_ids: Vec<String>,
    pub observed_edge_keys: Vec<String>,
}

/// Attach runtime observations to a static subgraph for UI highlighting.
pub fn attach_overlay(mut subgraph: Subgraph, overlay: &RuntimeOverlay) -> Subgraph {
    let mut observed_nodes = HashSet::new();
    for id in overlay.node_hits.keys() {
        observed_nodes.insert(id.clone());
    }
    for path in &overlay.paths {
        for id in &path.node_ids {
            observed_nodes.insert(id.clone());
        }
    }

    let mut observed_edges = HashSet::new();
    for path in &overlay.paths {
        for pair in path.node_ids.windows(2) {
            if let [from, to] = pair {
                if let Some(key) = find_edge_key(&subgraph.edges, from, to) {
                    observed_edges.insert(key);
                }
            }
        }
    }

    let mut node_hits = overlay.node_hits.clone();
    for path in &overlay.paths {
        for id in &path.node_ids {
            if path.hits > 0 {
                node_hits
                    .entry(id.clone())
                    .and_modify(|hits| *hits = (*hits).max(path.hits))
                    .or_insert(path.hits);
            }
        }
    }

    let mut observed_node_ids: Vec<_> = observed_nodes.into_iter().collect();
    observed_node_ids.sort();

    let mut observed_edge_keys: Vec<_> = observed_edges.into_iter().collect();
    observed_edge_keys.sort();

    subgraph.overlay = Some(SubgraphOverlay {
        node_hits,
        paths: overlay.paths.clone(),
        observed_node_ids,
        observed_edge_keys,
    });
    subgraph
}

fn find_edge_key(edges: &[GraphEdge], from: &str, to: &str) -> Option<String> {
    edges
        .iter()
        .find(|e| e.from_id == from && e.to_id == to)
        .map(|e| e.id.clone())
        .or_else(|| {
            edges
                .iter()
                .find(|e| e.from_id == to && e.to_id == from)
                .map(|e| e.id.clone())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_indexer::{GraphNode, NodeKind};

    #[test]
    fn attach_overlay_marks_path_nodes_and_edges() {
        let subgraph = Subgraph {
            nodes: vec![
                GraphNode {
                    id: "a".into(),
                    kind: NodeKind::Function,
                    name: "a".into(),
                    relative_path: "a.py".into(),
                    parent_file: Some("a.py".into()),
                    line: Some(1),
                    extension: None,
                    language_id: "python".into(),
                    size_bytes: None,
                },
                GraphNode {
                    id: "b".into(),
                    kind: NodeKind::Function,
                    name: "b".into(),
                    relative_path: "b.py".into(),
                    parent_file: Some("b.py".into()),
                    line: Some(1),
                    extension: None,
                    language_id: "python".into(),
                    size_bytes: None,
                },
            ],
            edges: vec![GraphEdge {
                id: "e1".into(),
                from_id: "a".into(),
                to_id: "b".into(),
                kind: graph_indexer::EdgeKind::Calls,
                confidence: Some(0.9),
            }],
            ghosts: vec![],
            overlay: None,
        };

        let overlay = RuntimeOverlay {
            schema_version: 1,
            node_hits: HashMap::from([("a".into(), 10)]),
            paths: vec![ObservedPath {
                label: Some("hot".into()),
                node_ids: vec!["a".into(), "b".into()],
                hits: 10,
            }],
        };

        let merged = attach_overlay(subgraph, &overlay);
        let meta = merged.overlay.expect("overlay");
        assert!(meta.observed_node_ids.contains(&"a".to_string()));
        assert!(meta.observed_node_ids.contains(&"b".to_string()));
        assert_eq!(meta.observed_edge_keys, vec!["e1"]);
        assert_eq!(meta.node_hits.get("a"), Some(&10));
    }
}
