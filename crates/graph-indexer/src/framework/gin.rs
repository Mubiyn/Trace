use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use regex::Regex;

/// Extract Gin `router.GET("/path", handler)` routes.
pub fn extract_routes(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r#"(?m)\b(?:router|r|engine)\.(GET|POST|PUT|DELETE|PATCH)\s*\(\s*["']([^"']+)["']"#,
    )
    .expect("valid regex");

    let mut symbols = Vec::new();
    for cap in re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let line = line_at(source, cap.get(0).unwrap().start());
        symbols.push(ExtractedSymbol {
            kind: NodeKind::Route,
            name: format!("{method} {path}"),
            parent_file: relative_path.to_string(),
            line,
            language_id: "go".to_string(),
        });
    }
    symbols
}

pub fn resolve_edges(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
    route_symbols: &[ExtractedSymbol],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let route_re = Regex::new(
        r#"(?m)\b(?:router|r|engine)\.(GET|POST|PUT|DELETE|PATCH)\s*\(\s*["']([^"']+)["']\s*,\s*([A-Za-z_][A-Za-z0-9_]*)"#,
    )
    .expect("valid regex");

    let mut edges = Vec::new();
    for cap in route_re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let handler = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
        let line = line_at(source, cap.get(0).unwrap().start());

        let route_name = format!("{method} {path}");
        let route_id = symbol_id(relative_path, NodeKind::Route, &route_name, line);
        if !route_symbols.iter().any(|s| {
            symbol_id(&s.parent_file, s.kind, &s.name, s.line) == route_id
        }) {
            continue;
        }

        if let Some(handler_id) = store.find_function_symbol_id(relative_path, handler) {
            edges.push(ResolvedEdge {
                from_id: route_id,
                to_id: handler_id,
                kind: EdgeKind::Handles,
                confidence: 0.88,
            });
        }
    }

    Ok(edges)
}

fn line_at(source: &str, offset: usize) -> u32 {
    source[..offset].chars().filter(|c| *c == '\n').count() as u32 + 1
}
