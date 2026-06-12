use super::module_path_to_file;
use crate::model::{EdgeKind, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const CALLS_QUERY: &str = r#"
(call
  function: (identifier) @callee) @call
"#;

const IMPORTS_QUERY: &str = r#"
(import_from_statement
  module_name: (_) @module
  name: (_) @import_name) @import_from

(import_from_statement
  module_name: (_) @module) @import_from_star
"#;

#[derive(Debug, Clone)]
struct ImportBinding {
    local_name: String,
    module: String,
    imported_name: String,
}

pub fn resolve_file(
    store: &GraphStore,
    root: &Path,
    relative_path: &str,
    source: &str,
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let Some(tree) = parser.parse(source, None) else {
        return Ok(Vec::new());
    };

    let bindings = parse_import_bindings(source, &tree, relative_path)?;
    let mut edges = Vec::new();

    for binding in &bindings {
        if let Some(target_id) = resolve_imported_symbol(store, root, relative_path, binding) {
            if let Some(import_id) =
                store.find_import_symbol_id(relative_path, &binding.local_name)
            {
                edges.push(ResolvedEdge {
                    from_id: import_id,
                    to_id: target_id.clone(),
                    kind: EdgeKind::Imports,
                    confidence: 0.92,
                });
            }

            let file_id = crate::model::node_id_for_path(relative_path);
            let target_file = module_path_to_file(relative_path, &binding.module);
            if let Some(target_file) = target_file {
                let target_file_id = crate::model::node_id_for_path(&target_file);
                if store.node_exists(&target_file_id) {
                    edges.push(ResolvedEdge {
                        from_id: file_id,
                        to_id: target_file_id,
                        kind: EdgeKind::Imports,
                        confidence: 0.85,
                    });
                }
            }
        }
    }

    edges.extend(resolve_calls(store, relative_path, source, &tree, &bindings)?);
    Ok(edges)
}

fn parse_import_bindings(
    source: &str,
    tree: &tree_sitter::Tree,
    relative_path: &str,
) -> Result<Vec<ImportBinding>, IndexError> {
    let language = tree_sitter_python::LANGUAGE.into();
    let query = Query::new(&language, IMPORTS_QUERY)
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut bindings = Vec::new();

    let mut matches = cursor.matches(&query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        let module_node = m.nodes_for_capture_index(0).next();
        let Some(module_node) = module_node else { continue };
        let Ok(module_text) = module_node.utf8_text(source.as_bytes()) else {
            continue;
        };
        let module = module_text.trim().to_string();

        if let Some(name_node) = m.nodes_for_capture_index(1).next() {
            let Ok(imported) = name_node.utf8_text(source.as_bytes()) else {
                continue;
            };
            let imported_name = imported.trim().to_string();
            bindings.push(ImportBinding {
                local_name: imported_name.clone(),
                module: module.clone(),
                imported_name,
            });
        } else {
            // from module import * — skip precise binding
            let _ = relative_path;
        }
    }

    // Also parse plain `from x import y` via import symbols already in graph
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("from ") {
            if let Some((module, names)) = rest.split_once(" import ") {
                let module = module.trim().to_string();
                for part in names.split(',') {
                    let part = part.trim();
                    if part == "*" || part.is_empty() {
                        continue;
                    }
                    let (imported_name, local_name) = if let Some((name, alias)) = part.split_once(" as ") {
                        (name.trim().to_string(), alias.trim().to_string())
                    } else {
                        (part.to_string(), part.to_string())
                    };
                    if bindings.iter().any(|b| b.local_name == local_name && b.module == module) {
                        continue;
                    }
                    bindings.push(ImportBinding {
                        local_name,
                        module: module.clone(),
                        imported_name,
                    });
                }
            }
        }
    }

    Ok(bindings)
}

fn resolve_imported_symbol(
    store: &GraphStore,
    _root: &Path,
    importer: &str,
    binding: &ImportBinding,
) -> Option<String> {
    let target_file = module_path_to_file(importer, &binding.module)?;
    store.find_function_symbol_id(&target_file, &binding.imported_name)
}

fn resolve_calls(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
    tree: &tree_sitter::Tree,
    bindings: &[ImportBinding],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let language = tree_sitter_python::LANGUAGE.into();
    let query =
        Query::new(&language, CALLS_QUERY).map_err(|e| IndexError::Parse(e.to_string()))?;
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut edges = Vec::new();

    let mut matches = cursor.matches(&query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        let Some(callee_node) = m.nodes_for_capture_index(0).next() else {
            continue;
        };
        let Some(call_node) = m.nodes_for_capture_index(1).next() else {
            continue;
        };
        if callee_node.kind() != "identifier" {
            continue;
        }
        let Ok(callee_name) = callee_node.utf8_text(source.as_bytes()) else {
            continue;
        };
        let callee_name = callee_name.trim();
        if is_builtin(callee_name) {
            continue;
        }

        let Some(caller_fn) = enclosing_function(call_node) else {
            continue;
        };
        let Some(caller_name) = function_name(caller_fn, source) else {
            continue;
        };
        let caller_line = caller_fn.start_position().row as u32 + 1;
        let caller_id = symbol_id(relative_path, NodeKind::Function, &caller_name, caller_line);

        let (target_id, confidence) = if let Some(id) =
            store.find_function_symbol_id(relative_path, callee_name)
        {
            (id, 0.95)
        } else if let Some(binding) = bindings.iter().find(|b| b.local_name == callee_name) {
            let Some(target_file) = module_path_to_file(relative_path, &binding.module) else {
                continue;
            };
            let Some(id) = store.find_function_symbol_id(&target_file, &binding.imported_name) else {
                continue;
            };
            (id, 0.9)
        } else {
            continue;
        };

        if !store.node_exists(&caller_id) {
            continue;
        }

        edges.push(ResolvedEdge {
            from_id: caller_id,
            to_id: target_id,
            kind: EdgeKind::Calls,
            confidence,
        });
    }

    Ok(edges)
}

fn enclosing_function(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(n) = current {
        if n.kind() == "function_definition" {
            return Some(n);
        }
        current = n.parent();
    }
    None
}

fn function_name(function_node: Node<'_>, source: &str) -> Option<String> {
    let name_node = function_node.child_by_field_name("name")?;
    let Ok(name) = name_node.utf8_text(source.as_bytes()) else {
        return None;
    };
    Some(name.trim().to_string())
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "print" | "len" | "range" | "str" | "int" | "float" | "bool" | "list" | "dict" | "set"
    )
}

#[cfg(test)]
mod tests {
    use crate::{fixture_path, index};

    #[test]
    fn resolves_python_simple_call_chain() {
        let path = fixture_path("python_simple");
        let result = index(&path).expect("index");
        let python = result
            .capabilities
            .languages
            .iter()
            .find(|l| l.id == "python")
            .expect("python");
        assert_eq!(python.tier, crate::model::Tier::L2);

        let json = crate::export_graph_json(&path).expect("export");
        let graph: serde_json::Value = serde_json::from_str(&json).unwrap();
        let edges = graph.get("edges").and_then(|e| e.as_array()).expect("edges");

        let calls: Vec<_> = edges
            .iter()
            .filter(|e| e.get("kind").and_then(|k| k.as_str()) == Some("CALLS"))
            .collect();
        assert!(
            calls.iter().any(|e| {
                e.get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .contains("greet")
                    && e.get("to")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .contains("format_message")
            }),
            "expected greet -> format_message CALLS edge"
        );
    }
}
