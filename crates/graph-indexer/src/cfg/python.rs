use crate::model::{EdgeKind, ExtractedSymbol, NodeKind, ResolvedEdge, symbol_id};
use crate::store::GraphStore;
use crate::IndexError;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const BRANCH_QUERY: &str = r#"
(if_statement) @branch
(try_statement) @branch
"#;

pub fn extract_branches(
    store: &GraphStore,
    relative_path: &str,
    source: &str,
) -> Result<(Vec<ExtractedSymbol>, Vec<ResolvedEdge>), IndexError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let Some(tree) = parser.parse(source, None) else {
        return Ok((Vec::new(), Vec::new()));
    };

    let query = Query::new(&tree_sitter_python::LANGUAGE.into(), BRANCH_QUERY)
        .map_err(|e| IndexError::Parse(e.to_string()))?;
    let mut cursor = QueryCursor::new();
    let mut branches = Vec::new();
    let mut edges = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(m) = matches.next() {
        let branch_node = m.nodes_for_capture_index(0).next().unwrap();
        let line = branch_node.start_position().row as u32 + 1;
        let name = branch_label(branch_node, source, line);

        branches.push(ExtractedSymbol {
            kind: NodeKind::Branch,
            name: name.clone(),
            parent_file: relative_path.to_string(),
            line,
            language_id: "python".to_string(),
        });

        let branch_id = symbol_id(relative_path, NodeKind::Branch, &name, line);
        if let Some(parent_fn) = enclosing_function_name(branch_node, source) {
            if let Some(parent_id) = store.find_function_symbol_id(relative_path, &parent_fn) {
                edges.push(ResolvedEdge {
                    from_id: parent_id,
                    to_id: branch_id.clone(),
                    kind: EdgeKind::BranchesTo,
                    confidence: 0.9,
                });
            }
        }

        for callee in callees_in_subtree(branch_node, source) {
            if let Some(target_id) = store.find_function_symbol_id(relative_path, &callee) {
                edges.push(ResolvedEdge {
                    from_id: branch_id.clone(),
                    to_id: target_id,
                    kind: EdgeKind::BranchesTo,
                    confidence: 0.87,
                });
            }
        }
    }

    Ok((branches, edges))
}

fn branch_label(node: Node<'_>, source: &str, line: u32) -> String {
    match node.kind() {
        "if_statement" => {
            if let Some(cond) = node.child_by_field_name("condition") {
                if let Ok(text) = cond.utf8_text(source.as_bytes()) {
                    let trimmed = text.trim().replace('\n', " ");
                    if trimmed.len() <= 48 {
                        return format!("if {trimmed}");
                    }
                    return format!("if {}…", &trimmed[..45]);
                }
            }
            format!("if @ line {line}")
        }
        "try_statement" => format!("try @ line {line}"),
        _ => format!("branch @ line {line}"),
    }
}

fn enclosing_function_name(mut node: Node<'_>, source: &str) -> Option<String> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "function_definition" {
            if let Some(name_node) = parent.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    return Some(name.to_string());
                }
            }
        }
        node = parent;
    }
    None
}

fn callees_in_subtree(node: Node<'_>, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect_calls(node, source, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_calls(node: Node<'_>, source: &str, out: &mut Vec<String>) {
    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "identifier" {
                if let Ok(name) = func.utf8_text(source.as_bytes()) {
                    out.push(name.to_string());
                }
            }
        }
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_calls(child, source, out);
        }
    }
}
