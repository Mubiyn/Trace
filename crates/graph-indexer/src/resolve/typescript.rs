use crate::model::{EdgeKind, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const CALLS_QUERY: &str = r#"
(call_expression
  function: (identifier) @callee) @call
"#;

#[derive(Clone)]
struct ImportBinding {
    local_name: String,
    module: String,
    imported_name: String,
}

pub fn resolve_file(
    store: &GraphStore,
    _root: &Path,
    relative_path: &str,
    source: &str,
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let Some(tree) = parser.parse(source, None) else {
        return Ok(Vec::new());
    };

    let bindings = parse_imports(source, &tree)?;
    let mut edges = Vec::new();

    for binding in &bindings {
        if let Some(target_file) = module_path_to_ts_file(relative_path, &binding.module) {
            if let Some(target_id) =
                store.find_function_symbol_id(&target_file, &binding.imported_name)
            {
                let file_id = crate::model::node_id_for_path(relative_path);
                let target_file_id = crate::model::node_id_for_path(&target_file);
                if store.node_exists(&target_file_id) {
                    edges.push(ResolvedEdge {
                        from_id: file_id,
                        to_id: target_file_id,
                        kind: EdgeKind::Imports,
                        confidence: 0.85,
                    });
                }
                if let Some(import_id) =
                    store.find_import_symbol_id(relative_path, &binding.local_name)
                {
                    edges.push(ResolvedEdge {
                        from_id: import_id,
                        to_id: target_id,
                        kind: EdgeKind::Imports,
                        confidence: 0.92,
                    });
                }
            }
        }
    }

    edges.extend(resolve_calls(store, relative_path, source, &tree, &bindings)?);
    Ok(edges)
}

fn parse_imports(source: &str, tree: &tree_sitter::Tree) -> Result<Vec<ImportBinding>, IndexError> {
    let mut bindings = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            if let Some((names, module)) = rest.split_once(" from ") {
                let module = module.trim().trim_matches(|c| c == '"' || c == '\'' || c == ';');
                let names = names
                    .trim()
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .trim();
                for part in names.split(',') {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }
                    let (imported, local) = if let Some((a, b)) = part.split_once(" as ") {
                        (a.trim().to_string(), b.trim().to_string())
                    } else {
                        (part.to_string(), part.to_string())
                    };
                    bindings.push(ImportBinding {
                        local_name: local,
                        module: module.to_string(),
                        imported_name: imported,
                    });
                }
            }
        }
    }

    let _ = tree;
    Ok(bindings)
}

fn resolve_calls(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
    tree: &tree_sitter::Tree,
    bindings: &[ImportBinding],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
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
        let Ok(callee_name) = callee_node.utf8_text(source.as_bytes()) else {
            continue;
        };
        let callee_name = callee_name.trim();

        let caller_id = if let Some(fn_node) = enclosing_function(call_node) {
            let Some(name) = function_name(fn_node, source) else {
                continue;
            };
            let line = fn_node.start_position().row as u32 + 1;
            symbol_id(relative_path, NodeKind::Function, &name, line)
        } else {
            continue;
        };

        let target_id = if let Some(id) = store.find_function_symbol_id(relative_path, callee_name) {
            id
        } else if let Some(binding) = bindings.iter().find(|b| b.local_name == callee_name) {
            let Some(target_file) = module_path_to_ts_file(relative_path, &binding.module) else {
                continue;
            };
            let Some(id) = store.find_function_symbol_id(&target_file, &binding.imported_name) else {
                continue;
            };
            id
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
            confidence: 0.9,
        });
    }

    Ok(edges)
}

fn enclosing_function(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(n) = current {
        if n.kind() == "function_declaration" {
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

pub fn module_path_to_ts_file(importer: &str, module: &str) -> Option<String> {
    if !module.starts_with('.') {
        return None;
    }
    let importer_path = Path::new(importer);
    let parent = importer_path.parent().unwrap_or(Path::new(""));
    let joined = parent.join(module.trim_start_matches("./"));
    if joined.components().any(|c| c.as_os_str() == "..") {
        return None;
    }
    let candidate = if joined.extension().is_some() {
        joined
    } else {
        joined.with_extension("ts")
    };
    Some(candidate.to_string_lossy().into_owned())
}
