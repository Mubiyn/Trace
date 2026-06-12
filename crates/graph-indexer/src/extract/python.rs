use crate::model::{ExtractedSymbol, NodeKind};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

const IMPORT_QUERY: &str = r#"
(import_statement) @import
(import_from_statement) @import
"#;

pub fn extract_imports(
    source: &str,
    relative_path: &str,
    language_id: &str,
    tree: tree_sitter::Tree,
) -> Vec<ExtractedSymbol> {
    let language = tree_sitter_python::LANGUAGE.into();
    let Ok(query) = Query::new(&language, IMPORT_QUERY) else {
        return Vec::new();
    };

    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();
    let root = tree.root_node();

    let mut matches = cursor.matches(&query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        let Some(node) = m.nodes_for_capture_index(0).next() else {
            continue;
        };
        let Ok(text) = node.utf8_text(source.as_bytes()) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let line = node.start_position().row as u32 + 1;
        symbols.push(ExtractedSymbol {
            kind: NodeKind::Import,
            name: text.to_string(),
            parent_file: relative_path.to_string(),
            line,
            language_id: language_id.to_string(),
        });
    }

    symbols
}
