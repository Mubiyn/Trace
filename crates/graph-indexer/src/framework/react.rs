use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::model::WalkEntry;
use crate::IndexError;
use regex::Regex;
use std::path::Path;

/// Extract React/JSX UI elements with `onClick` handlers.
pub fn extract_ui_elements(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r#"(?m)<([A-Za-z][A-Za-z0-9]*)[^>]*\bonClick=\{([A-Za-z_][A-Za-z0-9_]*)\}"#,
    )
    .expect("valid regex");

    let mut symbols = Vec::new();
    for cap in re.captures_iter(source) {
        let element = cap.get(1).map(|m| m.as_str()).unwrap_or("Element");
        let line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count() as u32
            + 1;
        let name = format!("{element}.onClick");
        symbols.push(ExtractedSymbol {
            kind: NodeKind::UiElement,
            name,
            parent_file: relative_path.to_string(),
            line,
            language_id: "typescript".to_string(),
        });
    }
    symbols
}

pub fn resolve_edges(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
    ui_symbols: &[ExtractedSymbol],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let re = Regex::new(
        r#"(?m)<([A-Za-z][A-Za-z0-9]*)[^>]*\bonClick=\{([A-Za-z_][A-Za-z0-9_]*)\}"#,
    )
    .expect("valid regex");

    let mut edges = Vec::new();
    for cap in re.captures_iter(source) {
        let element = cap.get(1).map(|m| m.as_str()).unwrap_or("Element");
        let handler = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
        let line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count() as u32
            + 1;

        let ui_id = symbol_id(
            relative_path,
            NodeKind::UiElement,
            &format!("{element}.onClick"),
            line,
        );
        if !ui_symbols.iter().any(|s| {
            symbol_id(&s.parent_file, s.kind, &s.name, s.line) == ui_id
        }) {
            continue;
        }

        if let Some(handler_id) = store.find_function_symbol_id(relative_path, handler) {
            edges.push(ResolvedEdge {
                from_id: ui_id.clone(),
                to_id: handler_id.clone(),
                kind: EdgeKind::Triggers,
                confidence: 0.88,
            });

            if let Some(component_id) = store.find_enclosing_function(relative_path, line) {
                if component_id != handler_id && component_id != ui_id {
                    edges.push(ResolvedEdge {
                        from_id: component_id,
                        to_id: handler_id,
                        kind: EdgeKind::Triggers,
                        confidence: 0.86,
                    });
                }
            }
        }
    }

    Ok(edges)
}

/// Link `fetch('/api/...')` call sites to `Route` nodes anywhere in the repo.
pub fn resolve_cross_layer_fetches(
    store: &GraphStore,
    root: &Path,
    entries: &[WalkEntry],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let fetch_re =
        Regex::new(r#"fetch\s*\(\s*['"]([^'"]+)['"]"#).expect("valid regex");
    let method_re = Regex::new(r#"method:\s*['"](GET|POST|PUT|DELETE|PATCH)['"]"#)
        .expect("valid regex");

    let mut edges = Vec::new();

    for entry in entries.iter().filter(|e| !e.is_dir) {
        let lang = crate::language::detect_language(&entry.relative_path);
        if lang != "typescript" && lang != "javascript" {
            continue;
        }
        let relative = entry.relative_path.to_string_lossy().into_owned();
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        for cap in fetch_re.captures_iter(&source) {
            let path = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let line = source[..cap.get(0).unwrap().start()]
                .chars()
                .filter(|c| *c == '\n')
                .count() as u32
                + 1;

            let window = source
                .lines()
                .nth(line.saturating_sub(1) as usize)
                .unwrap_or_default();
            let method = method_re
                .captures(window)
                .and_then(|m| m.get(1))
                .map(|m| m.as_str().to_uppercase())
                .unwrap_or_else(|| "GET".to_string());

            let route_name = format!("{method} {path}");
            let Some(route_id) = store.find_route_symbol_id(&route_name) else {
                continue;
            };

            let caller = enclosing_function_at_line(store, &relative, line)
                .unwrap_or_else(|| crate::model::node_id_for_path(&relative));

            edges.push(ResolvedEdge {
                from_id: caller,
                to_id: route_id,
                kind: EdgeKind::Fetches,
                confidence: 0.82,
            });
        }
    }

    Ok(edges)
}

fn enclosing_function_at_line(
    store: &GraphStore,
    file: &str,
    line: u32,
) -> Option<String> {
    store.find_enclosing_function(file, line)
}
