use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::model::WalkEntry;
use crate::store::GraphStore;
use crate::IndexError;
use regex::Regex;
use std::path::Path;

const WIDGET_RE: &str = r"(?s)([A-Za-z][A-Za-z0-9]*)\s*\(.*?\bon(?:Tap|Pressed):\s*(?:\(\)\s*=>\s*)?([A-Za-z_][A-Za-z0-9_]*)";

pub fn extract_ui_elements(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let widget_re = Regex::new(WIDGET_RE).expect("valid regex");

    let mut symbols = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in widget_re.captures_iter(source) {
        let widget = cap.get(1).map(|m| m.as_str()).unwrap_or("Widget");
        let line = line_at(source, cap.get(0).unwrap().start());
        let event = if source[cap.get(0).unwrap().range()].contains("onTap") {
            "onTap"
        } else {
            "onPressed"
        };
        let name = format!("{widget}.{event}");
        let key = (line, name.clone());
        if seen.insert(key) {
            symbols.push(ExtractedSymbol {
                kind: NodeKind::UiElement,
                name,
                parent_file: relative_path.to_string(),
                line,
                language_id: "dart".to_string(),
            });
        }
    }

    symbols
}

pub fn resolve_edges(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
    ui_symbols: &[ExtractedSymbol],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let widget_re = Regex::new(WIDGET_RE).expect("valid regex");
    let mut edges = Vec::new();

    for cap in widget_re.captures_iter(source) {
        let widget = cap.get(1).map(|m| m.as_str()).unwrap_or("Widget");
        let handler = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
        let line = line_at(source, cap.get(0).unwrap().start());
        let event = if source[cap.get(0).unwrap().range()].contains("onTap") {
            "onTap"
        } else {
            "onPressed"
        };
        let ui_name = format!("{widget}.{event}");
        let ui_id = symbol_id(relative_path, NodeKind::UiElement, &ui_name, line);
        if !ui_symbols.iter().any(|s| {
            symbol_id(&s.parent_file, s.kind, &s.name, s.line) == ui_id
        }) {
            continue;
        }
        if let Some(handler_id) = store.find_function_symbol_id(relative_path, handler) {
            edges.push(ResolvedEdge {
                from_id: ui_id,
                to_id: handler_id,
                kind: EdgeKind::Triggers,
                confidence: 0.86,
            });
        }
    }

    Ok(edges)
}

/// Link `http.post(Uri.parse('/api/...'))` in Dart to FastAPI/Express route nodes.
pub fn resolve_cross_layer_fetches(
    store: &GraphStore,
    root: &Path,
    entries: &[WalkEntry],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let fetch_re = Regex::new(
        r#"http\.(?:post|get|put|delete|patch)\s*\(\s*Uri\.parse\s*\(\s*['"]([^'"]+)['"]"#,
    )
    .expect("valid regex");

    let mut edges = Vec::new();
    for entry in entries.iter().filter(|e| !e.is_dir) {
        if crate::language::detect_language(&entry.relative_path) != "dart" {
            continue;
        }
        let relative = entry.relative_path.to_string_lossy().into_owned();
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };

        for cap in fetch_re.captures_iter(&source) {
            let path = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let line = line_at(&source, cap.get(0).unwrap().start());
            let window = source
                .lines()
                .nth(line.saturating_sub(1) as usize)
                .unwrap_or_default();
            let method = if window.contains("http.post") {
                "POST"
            } else if window.contains("http.get") {
                "GET"
            } else if window.contains("http.put") {
                "PUT"
            } else if window.contains("http.delete") {
                "DELETE"
            } else {
                "PATCH"
            };
            let route_name = format!("{method} {path}");
            let Some(route_id) = store.find_route_symbol_id(&route_name) else {
                continue;
            };
            let caller = store
                .find_enclosing_function(&relative, line)
                .unwrap_or_else(|| crate::model::node_id_for_path(&relative));
            edges.push(ResolvedEdge {
                from_id: caller,
                to_id: route_id,
                kind: EdgeKind::Fetches,
                confidence: 0.8,
            });
        }
    }

    Ok(edges)
}

fn line_at(source: &str, offset: usize) -> u32 {
    source[..offset].chars().filter(|c| *c == '\n').count() as u32 + 1
}
