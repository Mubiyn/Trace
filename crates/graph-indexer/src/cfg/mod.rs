mod python;

use crate::language::detect_language;
use crate::model::{ResolvedEdge, WalkEntry};
use crate::store::GraphStore;
use crate::IndexError;
use std::collections::HashSet;
use std::path::Path;

/// Control-flow branch extraction (decision → outcome edges).
pub fn resolve_cfg(
    store: &GraphStore,
    root: &Path,
    entries: &[WalkEntry],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let mut edges = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries.iter().filter(|e| !e.is_dir) {
        if detect_language(&entry.relative_path) != "python" {
            continue;
        }
        let relative = entry.relative_path.to_string_lossy().into_owned();
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        let (branches, branch_edges) = python::extract_branches(store, &relative, &source)?;
        store.insert_symbols(&branches)?;

        for edge in branch_edges {
            let key = (
                edge.from_id.clone(),
                edge.to_id.clone(),
                edge.kind.as_str().to_string(),
            );
            if seen.insert(key) {
                edges.push(edge);
            }
        }
    }

    Ok(edges)
}
