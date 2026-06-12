use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use regex::Regex;

/// Extract FastAPI/Flask-style route decorators.
pub fn extract_routes(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r#"(?m)@app\.(get|post|put|delete|patch)\(\s*['"]([^'"]+)['"]"#,
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
            language_id: "python".to_string(),
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
    let decorator_re = Regex::new(
        r#"(?m)@app\.(get|post|put|delete|patch)\(\s*['"]([^'"]+)['"]"#,
    )
    .expect("valid regex");
    let handler_re = Regex::new(r#"(?m)^def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("#)
        .expect("valid regex");

    let lines: Vec<&str> = source.lines().collect();
    let mut edges = Vec::new();

    for cap in decorator_re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let deco_line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count();

        let route_name = format!("{method} {path}");
        let route_id = symbol_id(
            relative_path,
            NodeKind::Route,
            &route_name,
            deco_line as u32 + 1,
        );

        if !route_symbols.iter().any(|s| {
            symbol_id(&s.parent_file, s.kind, &s.name, s.line) == route_id
        }) {
            continue;
        }

        let mut handler_name = None;
        for line in lines.iter().skip(deco_line + 1) {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if let Some(h) = handler_re.captures(line) {
                handler_name = h.get(1).map(|m| m.as_str().to_string());
                break;
            }
            break;
        }

        let Some(handler) = handler_name else {
            continue;
        };

        if let Some(handler_id) = store.find_function_symbol_id(relative_path, &handler) {
            edges.push(ResolvedEdge {
                from_id: route_id,
                to_id: handler_id,
                kind: EdgeKind::Handles,
                confidence: 0.93,
            });
        }
    }

    Ok(edges)
}
