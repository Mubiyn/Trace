use crate::model::{GraphNode, NodeKind};
use crate::store::GraphStore;
use crate::IndexError;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct ExportGraph<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    nodes: &'a [ExportNode<'a>],
    edges: &'a [ExportEdge<'a>],
}

#[derive(Serialize)]
struct ExportNode<'a> {
    id: &'a str,
    kind: &'a str,
    name: &'a str,
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    language: &'a str,
}

#[derive(Serialize)]
struct ExportEdge<'a> {
    #[serde(rename = "from")]
    from_id: &'a str,
    #[serde(rename = "to")]
    to_id: &'a str,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
}

impl GraphStore {
    pub fn export_graph_json(&self) -> Result<String, IndexError> {
        let nodes = self.list_nodes()?;
        let edges = self.list_edges()?;

        let export_nodes: Vec<ExportNode<'_>> = nodes
            .iter()
            .map(|n| ExportNode {
                id: &n.id,
                kind: n.kind.as_str(),
                name: &n.name,
                path: &n.relative_path,
                parent_file: n.parent_file.as_deref(),
                line: n.line,
                language: &n.language_id,
            })
            .collect();

        let export_edges: Vec<ExportEdge<'_>> = edges
            .iter()
            .map(|e| ExportEdge {
                from_id: &e.from_id,
                to_id: &e.to_id,
                kind: e.kind.as_str(),
                confidence: e.confidence,
            })
            .collect();

        let graph = ExportGraph {
            schema_version: 1,
            nodes: &export_nodes,
            edges: &export_edges,
        };

        serde_json::to_string_pretty(&graph).map_err(|e| IndexError::Export(e.to_string()))
    }
}

/// Compare two graph JSON values (order-independent).
pub fn graphs_equal(expected: &Value, actual: &Value) -> bool {
    normalize_graph(expected) == normalize_graph(actual)
}

fn normalize_graph(value: &Value) -> Value {
    let mut nodes = value
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let mut edges = value
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    nodes.sort_by(|a, b| {
        let aid = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let bid = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        aid.cmp(bid)
    });
    edges.sort_by(|a, b| {
        let ak = format!(
            "{}:{}:{}:{}",
            a.get("from").and_then(|v| v.as_str()).unwrap_or(""),
            a.get("to").and_then(|v| v.as_str()).unwrap_or(""),
            a.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
            a.get("confidence").map(|v| v.to_string()).unwrap_or_default()
        );
        let bk = format!(
            "{}:{}:{}:{}",
            b.get("from").and_then(|v| v.as_str()).unwrap_or(""),
            b.get("to").and_then(|v| v.as_str()).unwrap_or(""),
            b.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
            b.get("confidence").map(|v| v.to_string()).unwrap_or_default()
        );
        ak.cmp(&bk)
    });

    serde_json::json!({
        "schemaVersion": value.get("schemaVersion").or_else(|| value.get("schema_version")),
        "nodes": nodes,
        "edges": edges,
    })
}

pub fn count_symbols(nodes: &[GraphNode], kind: NodeKind) -> usize {
    nodes.iter().filter(|n| n.kind == kind).count()
}
