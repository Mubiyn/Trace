use crate::model::{EdgeKind, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const CALLS_QUERY: &str = r#"
(method_invocation
  name: (identifier) @callee) @call
"#;

pub fn resolve_file(
    store: &GraphStore,
    _root: &Path,
    relative_path: &str,
    source: &str,
    java_files: &[(String, String)],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let Some(tree) = parser.parse(source, None) else {
        return Ok(Vec::new());
    };

    let language = tree_sitter_java::LANGUAGE.into();
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

        let Some(fn_node) = enclosing_function(call_node) else {
            continue;
        };
        let Some(name) = function_name(fn_node, source) else {
            continue;
        };
        let line = fn_node.start_position().row as u32 + 1;
        let caller_id = symbol_id(relative_path, NodeKind::Function, &name, line);

        let Some(target_id) = find_java_method(java_files, store, callee_name) else {
            continue;
        };
        if !store.node_exists(&caller_id) {
            continue;
        }

        let confidence = if target_id.contains(relative_path) {
            0.94
        } else {
            0.86
        };
        edges.push(ResolvedEdge {
            from_id: caller_id,
            to_id: target_id,
            kind: EdgeKind::Calls,
            confidence,
        });
    }

    Ok(edges)
}

fn find_java_method(
    java_files: &[(String, String)],
    store: &GraphStore,
    name: &str,
) -> Option<String> {
    for (path, _) in java_files {
        if let Some(id) = store.find_function_symbol_id(path, name) {
            return Some(id);
        }
    }
    None
}

fn enclosing_function(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(n.kind(), "method_declaration" | "constructor_declaration") {
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
