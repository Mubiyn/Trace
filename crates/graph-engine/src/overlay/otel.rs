//! Convert OpenTelemetry trace JSON exports into graph runtime overlays.

use super::{ObservedPath, RuntimeOverlay};
use graph_indexer::GraphStore;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub enum OtelImportError {
    Parse(String),
    NoSpans,
}

impl std::fmt::Display for OtelImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OtelImportError::Parse(msg) => write!(f, "{msg}"),
            OtelImportError::NoSpans => write!(f, "no spans found in OpenTelemetry payload"),
        }
    }
}

impl std::error::Error for OtelImportError {}

/// Map OTEL JSON (OTLP export or flat `spans` array) to a [`RuntimeOverlay`].
pub fn otel_json_to_overlay(
    value: &Value,
    store: &GraphStore,
) -> Result<RuntimeOverlay, OtelImportError> {
    let spans = collect_spans(value);
    if spans.is_empty() {
        return Err(OtelImportError::NoSpans);
    }

    let mut node_hits: HashMap<String, u64> = HashMap::new();
    let mut paths_by_trace: HashMap<String, Vec<String>> = HashMap::new();

    for span in spans {
        let Some(node_id) = resolve_span_node(store, &span) else {
            continue;
        };
        *node_hits.entry(node_id.clone()).or_insert(0) += 1;

        let trace_id = span
            .get("traceId")
            .or_else(|| span.get("trace_id"))
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        paths_by_trace.entry(trace_id).or_default().push(node_id);
    }

    let paths = paths_by_trace
        .into_iter()
        .map(|(trace_id, node_ids)| ObservedPath {
            label: Some(format!("trace:{trace_id}")),
            node_ids,
            hits: 1,
        })
        .collect();

    Ok(RuntimeOverlay {
        schema_version: 1,
        node_hits,
        paths,
    })
}

fn collect_spans(value: &Value) -> Vec<Value> {
    if let Some(spans) = value.get("spans").and_then(Value::as_array) {
        return spans.clone();
    }

    let mut out = Vec::new();
    let Some(resource_spans) = value.get("resourceSpans").and_then(Value::as_array) else {
        return out;
    };

    for resource in resource_spans {
        let Some(scopes) = resource.get("scopeSpans").and_then(Value::as_array) else {
            continue;
        };
        for scope in scopes {
            if let Some(spans) = scope.get("spans").and_then(Value::as_array) {
                out.extend(spans.clone());
            }
        }
    }
    out
}

fn resolve_span_node(store: &GraphStore, span: &Value) -> Option<String> {
    let attrs = span_attributes(span);
    let function = attrs
        .get("code.function")
        .or_else(|| attrs.get("function.name"))
        .cloned();
    let file = attrs
        .get("code.filepath")
        .or_else(|| attrs.get("code.file.path"))
        .cloned();

    if let (Some(ref file_path), Some(ref function)) = (file.as_ref(), function.as_ref()) {
        let normalized = normalize_repo_path(file_path);
        if let Some(id) = store.find_function_symbol_id(&normalized, function) {
            return Some(id);
        }
    }

    if let Some(file) = file {
        let line = attrs
            .get("code.line.number")
            .and_then(|s| s.parse::<u32>().ok())
            .or_else(|| {
                span
                    .get("startLine")
                    .or_else(|| span.get("start_line"))
                    .and_then(Value::as_u64)
                    .map(|n| n as u32)
            });
        let normalized = normalize_repo_path(&file);
        if let Some(line) = line {
            if let Some(id) = store.find_enclosing_function(&normalized, line) {
                return Some(id);
            }
        }
    }

    if let Some(name) = span.get("name").and_then(Value::as_str) {
        return store
            .search_symbols(name, 5)
            .ok()
            .and_then(|nodes| nodes.into_iter().find(|n| n.name == name).map(|n| n.id));
    }

    None
}

fn span_attributes(span: &Value) -> HashMap<String, String> {
    let mut out = HashMap::new();

    if let Some(map) = span.get("attributes").and_then(Value::as_object) {
        for (key, value) in map {
            if let Some(s) = value.as_str() {
                out.insert(key.clone(), s.to_string());
            }
        }
    }

    if let Some(attrs) = span.get("attributes").and_then(Value::as_array) {
        for attr in attrs {
            let Some(key) = attr.get("key").and_then(Value::as_str) else {
                continue;
            };
            let value = attr
                .get("value")
                .and_then(|v| {
                    v.get("stringValue")
                        .or_else(|| v.get("intValue"))
                        .or_else(|| v.get("boolValue"))
                })
                .map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        v.to_string()
                    }
                });
            if let Some(value) = value {
                out.insert(key.to_string(), value);
            }
        }
    }

    out
}

fn normalize_repo_path(path: &str) -> String {
    path.trim_start_matches('/')
        .trim_start_matches("./")
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_indexer::fixture_path;

    #[test]
    fn otel_export_maps_to_python_simple_symbols() {
        let store = graph_indexer::indexed_store(&fixture_path("python_simple")).expect("index");
        let json: Value = serde_json::from_str(include_str!(
            "../../../../fixtures/otel_trace/export.json"
        ))
        .expect("fixture json");

        let overlay = otel_json_to_overlay(&json, &store).expect("import");
        assert!(overlay.node_hits.len() >= 2);
        assert!(!overlay.paths.is_empty());
    }
}
