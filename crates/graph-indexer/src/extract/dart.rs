use crate::model::{ExtractedSymbol, NodeKind};
use regex::Regex;

/// Lightweight L1 for Dart until a tree-sitter grammar is bundled.
pub fn extract_symbols(source: &str, relative_path: &str) -> Vec<ExtractedSymbol> {
    let re = Regex::new(
        r"(?m)^(?:\s*)(?:[\w<>?.,\s]+)\s+(\w+)\s*\([^)]*\)\s*(?:async\s*)?\{",
    )
    .expect("valid regex");

    let mut symbols = Vec::new();
    for cap in re.captures_iter(source) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        if matches!(name, "if" | "for" | "while" | "switch" | "catch") {
            continue;
        }
        let line = source[..cap.get(0).unwrap().start()]
            .chars()
            .filter(|c| *c == '\n')
            .count() as u32
            + 1;
        symbols.push(ExtractedSymbol {
            kind: NodeKind::Function,
            name: name.to_string(),
            parent_file: relative_path.to_string(),
            line,
            language_id: "dart".to_string(),
        });
    }
    symbols
}
