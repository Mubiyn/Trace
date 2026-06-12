use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use regex::Regex;

/// Extract Spring MVC `@GetMapping` / `@PostMapping` route annotations.
pub fn extract_routes(relative_path: &str, source: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r#"(?m)@(Get|Post|Put|Delete|Patch)Mapping\s*\(\s*["']([^"']+)["']"#,
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
            language_id: "java".to_string(),
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
        r#"(?m)@(Get|Post|Put|Delete|Patch)Mapping\s*\(\s*["']([^"']+)["']"#,
    )
    .expect("valid regex");
    let handler_re =
        Regex::new(r#"(?m)^\s*(?:public|private|protected)?\s+[\w<>,\[\]\s]+\s+(\w+)\s*\("#)
            .expect("valid regex");

    let lines: Vec<&str> = source.lines().collect();
    let mut edges = Vec::new();

    for cap in decorator_re.captures_iter(source) {
        let method = cap
            .get(1)
            .map(|m| m.as_str().to_uppercase())
            .unwrap_or_else(|| "GET".to_string());
        let path = cap.get(2).map(|m| m.as_str()).unwrap_or("/");
        let deco_line = line_at(source, cap.get(0).unwrap().start()) as usize;

        let route_name = format!("{method} {path}");
        let route_id = symbol_id(relative_path, NodeKind::Route, &route_name, deco_line as u32);
        if !route_symbols.iter().any(|s| {
            symbol_id(&s.parent_file, s.kind, &s.name, s.line) == route_id
        }) {
            continue;
        }

        let handler = lines
            .iter()
            .skip(deco_line)
            .find_map(|line| handler_re.captures(line))
            .and_then(|m| m.get(1).map(|g| g.as_str().to_string()));

        let Some(handler_name) = handler else {
            continue;
        };
        let Some(handler_id) = store.find_function_symbol_id(relative_path, &handler_name) else {
            continue;
        };

        edges.push(ResolvedEdge {
            from_id: route_id,
            to_id: handler_id,
            kind: EdgeKind::Handles,
            confidence: 0.87,
        });
    }

    Ok(edges)
}

fn line_at(source: &str, offset: usize) -> u32 {
    source[..offset].chars().filter(|c| *c == '\n').count() as u32 + 1
}
