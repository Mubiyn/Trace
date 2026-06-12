mod dart;
mod python;

use crate::model::{ExtractedSymbol, NodeKind};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

type ExtraSymbolExtractor =
    fn(&str, &str, &str, tree_sitter::Tree) -> Vec<ExtractedSymbol>;

struct LangQuery {
    language_id: &'static str,
    language: Language,
    /// tree-sitter query; captures named `name` on function/class nodes.
    symbols_query: &'static str,
    /// Optional extra query (e.g. Python imports).
    extra: Option<ExtraSymbolExtractor>,
}

fn lang_specs() -> Vec<LangQuery> {
    vec![
        LangQuery {
            language_id: "python",
            language: tree_sitter_python::LANGUAGE.into(),
            symbols_query: r#"
                (function_definition
                  name: (identifier) @name) @item
                (class_definition
                  name: (identifier) @name) @item
            "#,
            extra: Some(python::extract_imports),
        },
        LangQuery {
            language_id: "rust",
            language: tree_sitter_rust::LANGUAGE.into(),
            symbols_query: r#"
                (function_item
                  name: (identifier) @name) @item
                (struct_item
                  name: (type_identifier) @name) @item
                (enum_item
                  name: (type_identifier) @name) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "go",
            language: tree_sitter_go::LANGUAGE.into(),
            symbols_query: r#"
                (function_declaration
                  name: (identifier) @name) @item
                (method_declaration
                  name: (field_identifier) @name) @item
                (type_declaration
                  (type_spec
                    name: (type_identifier) @name)) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "javascript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            symbols_query: r#"
                (function_declaration
                  name: (identifier) @name) @item
                (export_statement
                  declaration: (function_declaration
                    name: (identifier) @name)) @item
                (class_declaration
                  name: (identifier) @name) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "typescript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            symbols_query: "(function_declaration name: (identifier) @name) @item",
            extra: None,
        },
        LangQuery {
            language_id: "ruby",
            language: tree_sitter_ruby::LANGUAGE.into(),
            symbols_query: r#"
                (method
                  name: (identifier) @name) @item
                (singleton_method
                  name: (identifier) @name) @item
                (class
                  name: (constant) @name) @item
                (module
                  name: (constant) @name) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "java",
            language: tree_sitter_java::LANGUAGE.into(),
            symbols_query: r#"
                (method_declaration
                  name: (identifier) @name) @item
                (constructor_declaration
                  name: (identifier) @name) @item
                (class_declaration
                  name: (identifier) @name) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "php",
            language: tree_sitter_php::LANGUAGE_PHP.into(),
            symbols_query: r#"
                (function_definition
                  name: (name) @name) @item
                (method_declaration
                  name: (name) @name) @item
                (class_declaration
                  name: (name) @name) @item
            "#,
            extra: None,
        },
        LangQuery {
            language_id: "swift",
            language: tree_sitter_swift::LANGUAGE.into(),
            symbols_query: r#"
                (function_declaration
                  name: (simple_identifier) @name) @item
                (class_declaration
                  name: (type_identifier) @name) @item
            "#,
            extra: None,
        },
    ]
}

/// Extract structural symbols from a source file. Returns empty on unsupported language or parse errors.
pub fn extract_file(
    language_id: &str,
    source: &str,
    relative_path: &str,
) -> Vec<ExtractedSymbol> {
    if language_id == "dart" {
        return dart::extract_symbols(source, relative_path);
    }

    let Some(spec) = lang_specs()
        .into_iter()
        .find(|s| s.language_id == language_id)
    else {
        return Vec::new();
    };

    extract_with_spec(&spec, source, relative_path).unwrap_or_default()
}

fn extract_with_spec(
    spec: &LangQuery,
    source: &str,
    relative_path: &str,
) -> Option<Vec<ExtractedSymbol>> {
    let mut parser = Parser::new();
    parser.set_language(&spec.language).ok()?;
    let tree = parser.parse(source, None)?;

    let mut symbols = extract_named_items(spec, source, relative_path, &tree)?;

    if let Some(extra) = spec.extra {
        symbols.extend(extra(source, relative_path, spec.language_id, tree));
    }

    symbols.sort_by(|a, b| {
        a.parent_file
            .cmp(&b.parent_file)
            .then(a.line.cmp(&b.line))
            .then(a.name.cmp(&b.name))
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });
    symbols.dedup_by(|a, b| {
        a.parent_file == b.parent_file && a.kind == b.kind && a.name == b.name && a.line == b.line
    });

    Some(symbols)
}

fn extract_named_items(
    spec: &LangQuery,
    source: &str,
    relative_path: &str,
    tree: &tree_sitter::Tree,
) -> Option<Vec<ExtractedSymbol>> {
    let query = Query::new(&spec.language, spec.symbols_query).ok()?;
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();
    let root = tree.root_node();

    let mut matches = cursor.matches(&query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        let (item_node, name_node) = match_nodes(m, source)?;

        let name = name_node
            .utf8_text(source.as_bytes())
            .ok()
            .map(str::trim)
            .filter(|s| !s.is_empty())?;

        let kind = classify_symbol(item_node.kind());
        let line = name_node.start_position().row as u32 + 1;

        symbols.push(ExtractedSymbol {
            kind,
            name: name.to_string(),
            parent_file: relative_path.to_string(),
            line,
            language_id: spec.language_id.to_string(),
        });
    }

    Some(symbols)
}

fn match_nodes<'a>(
    m: &tree_sitter::QueryMatch<'a, 'a>,
    source: &'a str,
) -> Option<(tree_sitter::Node<'a>, tree_sitter::Node<'a>)> {
    let mut item = None;
    let mut name = None;

    for i in 0..m.captures.len() {
        let node = m.nodes_for_capture_index(i as u32).next()?;
        if is_name_node(node) {
            name = Some(node);
        } else {
            item = Some(node);
        }
    }

    let name_node = name?;
    let item_node = item.unwrap_or(name_node);
    let _ = source; // kept for future validation
    Some((item_node, name_node))
}

fn is_name_node(node: tree_sitter::Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "simple_identifier"
            | "type_identifier"
            | "constant"
            | "field_identifier"
            | "property_identifier"
            | "name"
    )
}

fn classify_symbol(kind: &str) -> NodeKind {
    match kind {
        "class_definition" | "class_declaration" | "struct_item" | "enum_item"
        | "type_declaration" | "class" | "module" => NodeKind::Class,
        _ => NodeKind::Function,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_typescript_export_functions() {
        let source = "export function handle() {}\nfunction respond() {}";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .expect("set language");
        let tree = parser.parse(source, None).expect("parse");
        assert!(
            !tree.root_node().has_error(),
            "parse errors: {}",
            source
        );

        let symbols = extract_file("typescript", source, "service.ts");
        assert!(
            symbols.iter().any(|s| s.name == "handle"),
            "symbols: {symbols:?}"
        );
        assert!(
            symbols.iter().any(|s| s.name == "respond"),
            "symbols: {symbols:?}"
        );
    }

    #[test]
    fn extracts_php_functions() {
        let source = r#"<?php
function greet($name) {
    return format_message($name);
}
"#;
        let symbols = extract_file("php", source, "index.php");
        assert!(
            symbols.iter().any(|s| s.name == "greet"),
            "symbols: {symbols:?}"
        );
    }

    #[test]
    fn extracts_swift_functions() {
        let source = r#"func greet(_ name: String) -> String {
    return formatMessage(name)
}
"#;
        let symbols = extract_file("swift", source, "main.swift");
        assert!(
            symbols.iter().any(|s| s.name == "greet"),
            "symbols: {symbols:?}"
        );
    }

    #[test]
    fn extracts_python_functions_and_imports() {
        let source = r#"from utils import format_message

def greet(name: str) -> str:
    return format_message(name)
"#;
        let symbols = extract_file("python", source, "main.py");
        let kinds: Vec<_> = symbols.iter().map(|s| (s.kind, s.name.as_str())).collect();
        assert!(kinds.iter().any(|(k, n)| *k == NodeKind::Function && *n == "greet"));
        assert!(kinds.iter().any(|(k, _)| *k == NodeKind::Import));
    }
}
