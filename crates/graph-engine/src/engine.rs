use crate::EngineError;
use graph_indexer::{
    Capabilities, EdgeKind, GraphEdge, GraphNode, GraphStore, NodeKind, indexed_store, symbol_id,
};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::path::Path;

/// Query interface over an indexed graph.
pub struct GraphEngine {
    store: GraphStore,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Neighbor {
    pub node: GraphNode,
    pub edge: GraphEdge,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TraceHop {
    pub node: GraphNode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<GraphEdge>,
    pub siblings: Vec<GraphNode>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TraceResult {
    pub hops: Vec<TraceHop>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BoundaryDirection {
    Ingress,
    Egress,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GhostNode {
    pub id: String,
    pub name: String,
    pub direction: BoundaryDirection,
    pub via: GraphEdge,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Subgraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub ghosts: Vec<GhostNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<crate::overlay::SubgraphOverlay>,
}

impl GraphEngine {
    pub fn from_store(store: GraphStore) -> Self {
        Self { store }
    }

    pub fn index(path: &Path) -> Result<Self, EngineError> {
        Ok(Self {
            store: indexed_store(path)?,
        })
    }

    /// Open a previously persisted SQLite index (hosted / on-disk).
    pub fn open_persisted(db_path: &Path) -> Result<Self, EngineError> {
        Ok(Self {
            store: GraphStore::open(db_path)?,
        })
    }

    pub fn store(&self) -> &GraphStore {
        &self.store
    }

    pub fn capabilities(&self) -> Result<Capabilities, EngineError> {
        Ok(self.store.capabilities()?)
    }

    pub fn get_node(&self, id: &str) -> Result<Option<GraphNode>, EngineError> {
        Ok(self.store.get_node(id)?)
    }

    pub fn require_node(&self, id: &str) -> Result<GraphNode, EngineError> {
        self.store
            .get_node(id)?
            .ok_or_else(|| EngineError::NotFound(id.to_string()))
    }

    /// Fuzzy search over symbol names, paths, and ids.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<GraphNode>, EngineError> {
        Ok(self.store.search_symbols(query, limit)?)
    }

    /// Symbols that call `id` via `CALLS` edges.
    pub fn callers(&self, id: &str) -> Result<Vec<Neighbor>, EngineError> {
        self.require_node(id)?;
        self.neighbors_to(id, EdgeKind::Calls)
    }

    /// Symbols called by `id` via `CALLS` edges.
    pub fn callees(&self, id: &str) -> Result<Vec<Neighbor>, EngineError> {
        self.require_node(id)?;
        self.neighbors_from(id, EdgeKind::Calls)
    }

    /// Blast radius: nodes that transitively call `id` up to `depth` (reverse `CALLS` BFS).
    pub fn impact(&self, id: &str, depth: usize) -> Result<Vec<GraphNode>, EngineError> {
        self.require_node(id)?;
        let mut seen = HashSet::from([id.to_string()]);
        let mut queue = VecDeque::from([(id.to_string(), 0usize)]);
        let mut impacted = Vec::new();

        while let Some((current, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }
            for edge in self.store.edges_to(&current, Some(EdgeKind::Calls))? {
                if seen.insert(edge.from_id.clone()) {
                    if let Some(node) = self.store.get_node(&edge.from_id)? {
                        impacted.push(node);
                        queue.push_back((edge.from_id, d + 1));
                    }
                }
            }
        }

        impacted.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(impacted)
    }

    /// Entry points: symbols with no incoming `CALLS`, plus common root names.
    pub fn entry_points(&self) -> Result<Vec<GraphNode>, EngineError> {
        let mut roots = self.store.symbols_without_incoming_calls()?;
        let mut seen: HashSet<String> = roots.iter().map(|n| n.id.clone()).collect();

        for node in self.store.search_symbols("main", 50)? {
            if node.kind == NodeKind::Function && seen.insert(node.id.clone()) {
                roots.push(node);
            }
        }

        for node in self.store.list_nodes()? {
            if node.kind == NodeKind::UiElement && seen.insert(node.id.clone()) {
                roots.push(node);
            }
        }

        roots.sort_by(|a, b| {
            a.relative_path
                .cmp(&b.relative_path)
                .then(a.line.unwrap_or(0).cmp(&b.line.unwrap_or(0)))
        });
        Ok(roots)
    }

    /// Follow `CALLS` edges from `root_id` up to `max_depth`. Siblings = alternate callees.
    pub fn trace(&self, root_id: &str, max_depth: usize) -> Result<TraceResult, EngineError> {
        self.require_node(root_id)?;
        let mut hops = Vec::new();
        let mut current_id = root_id.to_string();
        let mut prev_id: Option<String> = None;
        let mut visited = HashSet::from([root_id.to_string()]);

        for _ in 0..max_depth {
            let node = self.require_node(&current_id)?;
            let outgoing = self.trace_outgoing_edges(&current_id)?;

            let via = match &prev_id {
                Some(prev) => self
                    .trace_outgoing_edges(prev)?
                    .into_iter()
                    .find(|e| e.to_id == current_id),
                None => None,
            };

            let mut ranked: Vec<(GraphEdge, GraphNode)> = Vec::new();
            for edge in outgoing {
                if let Some(target) = self.store.get_node(&edge.to_id)? {
                    ranked.push((edge, target));
                }
            }
            ranked.sort_by(|a, b| {
                let a_branch = a.1.kind == NodeKind::Branch;
                let b_branch = b.1.kind == NodeKind::Branch;
                b_branch
                    .cmp(&a_branch)
                    .then_with(|| {
                        let ac = a.0.confidence.unwrap_or(0.0);
                        let bc = b.0.confidence.unwrap_or(0.0);
                        bc.partial_cmp(&ac)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then(a.1.id.cmp(&b.1.id))
            });

            let siblings = ranked.iter().skip(1).map(|(_, n)| n.clone()).collect();
            let next = ranked.first().map(|(e, n)| (e.to_id.clone(), n.id.clone()));

            hops.push(TraceHop {
                node,
                via,
                siblings,
            });

            let Some((next_id, _)) = next else {
                break;
            };
            if !visited.insert(next_id.clone()) {
                break;
            }
            prev_id = Some(current_id);
            current_id = next_id;
        }

        Ok(TraceResult { hops })
    }

    /// Nodes within `scope` (file path or directory prefix) plus boundary ghost nodes.
    pub fn subgraph(&self, scope: &str, include_boundary: bool) -> Result<Subgraph, EngineError> {
        let scope = scope.trim_matches('/');
        let all_nodes = self.store.list_nodes()?;
        let all_edges = self.store.list_edges()?;

        let in_scope = |node: &GraphNode| -> bool {
            if scope.is_empty() || scope == "." {
                return true;
            }
            let path = if node.parent_file.is_some() {
                node.parent_file.as_deref().unwrap_or(&node.relative_path)
            } else {
                &node.relative_path
            };
            path == scope || path.starts_with(&format!("{scope}/"))
        };

        let internal_ids: HashSet<String> = all_nodes
            .iter()
            .filter(|n| in_scope(n))
            .map(|n| n.id.clone())
            .collect();

        let nodes: Vec<GraphNode> = all_nodes
            .into_iter()
            .filter(|n| internal_ids.contains(&n.id))
            .collect();

        let mut edges = Vec::new();
        let mut ghosts = Vec::new();

        for edge in all_edges {
            let from_in = internal_ids.contains(&edge.from_id);
            let to_in = internal_ids.contains(&edge.to_id);

            if from_in && to_in {
                edges.push(edge);
            } else if include_boundary && (from_in || to_in) {
                if from_in && !to_in {
                    if let Some(ext) = self.store.get_node(&edge.to_id)? {
                        ghosts.push(GhostNode {
                            id: format!("ghost:egress:{}", ext.id),
                            name: ext.name.clone(),
                            direction: BoundaryDirection::Egress,
                            via: edge,
                        });
                    }
                } else if !from_in && to_in {
                    if let Some(ext) = self.store.get_node(&edge.from_id)? {
                        ghosts.push(GhostNode {
                            id: format!("ghost:ingress:{}", ext.id),
                            name: ext.name.clone(),
                            direction: BoundaryDirection::Ingress,
                            via: edge,
                        });
                    }
                }
            }
        }

        ghosts.sort_by(|a, b| a.id.cmp(&b.id));
        edges.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Subgraph {
            nodes,
            edges,
            ghosts,
            overlay: None,
        })
    }

    /// Resolve a symbol by file path and function name.
    pub fn symbol_id_for(&self, file: &str, name: &str, line: u32) -> Option<String> {
        let id = symbol_id(file, NodeKind::Function, name, line);
        self.store.get_node(&id).ok().flatten().map(|n| n.id)
    }

    fn trace_outgoing_edges(&self, id: &str) -> Result<Vec<GraphEdge>, EngineError> {
        let mut edges = Vec::new();
        for kind in [
            EdgeKind::Triggers,
            EdgeKind::BranchesTo,
            EdgeKind::Calls,
            EdgeKind::Fetches,
            EdgeKind::Handles,
        ] {
            edges.extend(self.store.edges_from(id, Some(kind))?);
        }
        Ok(edges)
    }

    fn neighbors_from(&self, id: &str, kind: EdgeKind) -> Result<Vec<Neighbor>, EngineError> {
        let mut out = Vec::new();
        for edge in self.store.edges_from(id, Some(kind))? {
            if let Some(node) = self.store.get_node(&edge.to_id)? {
                out.push(Neighbor { node, edge });
            }
        }
        out.sort_by(|a, b| {
            let ac = a.edge.confidence.unwrap_or(0.0);
            let bc = b.edge.confidence.unwrap_or(0.0);
            bc.partial_cmp(&ac)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.node.id.cmp(&b.node.id))
        });
        Ok(out)
    }

    fn neighbors_to(&self, id: &str, kind: EdgeKind) -> Result<Vec<Neighbor>, EngineError> {
        let mut out = Vec::new();
        for edge in self.store.edges_to(id, Some(kind))? {
            if let Some(node) = self.store.get_node(&edge.from_id)? {
                out.push(Neighbor { node, edge });
            }
        }
        out.sort_by(|a, b| {
            let ac = a.edge.confidence.unwrap_or(0.0);
            let bc = b.edge.confidence.unwrap_or(0.0);
            bc.partial_cmp(&ac)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.node.id.cmp(&b.node.id))
        });
        Ok(out)
    }
}
