use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use regex::Regex;

/// Extract Express.js route registrations (`app.get('/path', handler)`).
pub fn extract_routes(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r#"(?m)\bapp\.(get|post|put|delete|patch)\(\s*['"]([^'"]+)['"]"#,
    )
    .expect("valid regex");

    let mut symbols = Vec::new();
    for cap in re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count() as u32
            + 1;
        symbols.push(ExtractedSymbol {
            kind: NodeKind::Route,
            name: format!("{method} {path}"),
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
    route_symbols: &[ExtractedSymbol],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let re = Regex::new(
        r#"(?m)\bapp\.(get|post|put|delete|patch)\(\s*['"]([^'"]+)['"]\s*,\s*([A-Za-z_][A-Za-z0-9_]*)"#,
    )
    .expect("valid regex");

    let mut edges = Vec::new();
    for cap in re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let handler = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
        let line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count() as u32
            + 1;

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
                confidence: 0.91,
            });
        }
    }

    Ok(edges)
}
