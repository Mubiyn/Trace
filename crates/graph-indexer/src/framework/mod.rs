mod actix;
mod django;
mod express;
mod fastapi;
mod flask;
mod flutter;
mod gin;
mod react;
mod spring;

use crate::language::detect_language;
use crate::model::{ResolvedEdge, WalkEntry};
use crate::store::GraphStore;
use crate::IndexError;
use std::collections::HashSet;
use std::path::Path;

/// L3/L4 framework wiring after symbols and L2 relations are indexed.
pub fn resolve_framework(
    store: &GraphStore,
    root: &Path,
    entries: &[WalkEntry],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let mut edges = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries.iter().filter(|e| !e.is_dir) {
        let relative = entry.relative_path.to_string_lossy().into_owned();
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        let lang = detect_language(&entry.relative_path);
        let ui_symbols = match lang {
            "typescript" | "javascript" => react::extract_ui_elements(&relative, &source),
            "dart" => flutter::extract_ui_elements(&relative, &source),
            _ => Vec::new(),
        };
        store.insert_symbols(&ui_symbols)?;

        let mut route_symbols = match lang {
            "typescript" | "javascript" => express::extract_routes(&relative, &source),
            "java" => spring::extract_routes(&relative, &source),
            "go" => gin::extract_routes(&relative, &source),
            "rust" => actix::extract_routes(&relative, &source),
            _ => Vec::new(),
        };
        if lang == "python" {
            route_symbols.extend(fastapi::extract_routes(&relative, &source));
            route_symbols.extend(flask::extract_routes(&relative, &source));
            route_symbols.extend(django::extract_routes(&relative, &source));
        }
        store.insert_symbols(&route_symbols)?;

        let mut file_edges = match lang {
            "java" => spring::resolve_edges(store, &relative, &source, &route_symbols)?,
            "go" => gin::resolve_edges(store, &relative, &source, &route_symbols)?,
            "rust" => actix::resolve_edges(store, &relative, &source, &route_symbols)?,
            _ => Vec::new(),
        };
        if lang == "python" {
            file_edges.extend(fastapi::resolve_edges(store, &relative, &source, &route_symbols)?);
            file_edges.extend(flask::resolve_edges(store, &relative, &source, &route_symbols)?);
            file_edges.extend(django::resolve_edges(store, &relative, &source, &route_symbols)?);
        }
        if lang == "typescript" || lang == "javascript" {
            file_edges.extend(react::resolve_edges(store, &relative, &source, &ui_symbols)?);
            file_edges.extend(express::resolve_edges(store, &relative, &source, &route_symbols)?);
        }
        if lang == "dart" {
            file_edges.extend(flutter::resolve_edges(store, &relative, &source, &ui_symbols)?);
        }

        for edge in file_edges {
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

    edges.extend(react::resolve_cross_layer_fetches(store, root, entries)?);
    edges.extend(flutter::resolve_cross_layer_fetches(store, root, entries)?);

    Ok(edges)
}
