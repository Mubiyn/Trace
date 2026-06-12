mod go;
mod java;
mod php;
mod python;
mod rust;
mod swift;
mod typescript;

use crate::language::detect_language;
use crate::model::{ResolvedEdge, WalkEntry};
use crate::store::GraphStore;
use crate::IndexError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Resolve CALLS and IMPORTS edges for all indexed source files.
pub fn resolve_relations(
    store: &GraphStore,
    root: &Path,
    entries: &[WalkEntry],
) -> Result<Vec<ResolvedEdge>, IndexError> {
    let sources = load_sources(root, entries)?;
    let go_files: Vec<_> = sources
        .iter()
        .filter(|(p, _)| detect_language(Path::new(p)) == "go")
        .cloned()
        .collect();
    let rust_files: Vec<_> = sources
        .iter()
        .filter(|(p, _)| detect_language(Path::new(p)) == "rust")
        .cloned()
        .collect();
    let java_files: Vec<_> = sources
        .iter()
        .filter(|(p, _)| detect_language(Path::new(p)) == "java")
        .cloned()
        .collect();
    let php_files: Vec<_> = sources
        .iter()
        .filter(|(p, _)| detect_language(Path::new(p)) == "php")
        .cloned()
        .collect();
    let swift_files: Vec<_> = sources
        .iter()
        .filter(|(p, _)| detect_language(Path::new(p)) == "swift")
        .cloned()
        .collect();

    let mut edges = Vec::new();
    let mut seen = HashSet::new();

    for (relative_path, source) in sources {
        let lang = detect_language(Path::new(&relative_path));
        let file_edges = match lang {
            "python" => python::resolve_file(store, root, &relative_path, &source)?,
            "go" => go::resolve_file(store, root, &relative_path, &source, &go_files)?,
            "rust" => rust::resolve_file(store, root, &relative_path, &source, &rust_files)?,
            "typescript" | "javascript" => {
                typescript::resolve_file(store, root, &relative_path, &source)?
            }
            "java" => java::resolve_file(store, root, &relative_path, &source, &java_files)?,
            "php" => php::resolve_file(store, root, &relative_path, &source, &php_files)?,
            "swift" => swift::resolve_file(store, root, &relative_path, &source, &swift_files)?,
            _ => Vec::new(),
        };

        for edge in file_edges {
            let key = (
                edge.from_id.clone(),
                edge.to_id.clone(),
                edge.kind.as_str().to_string(),
            );
            if seen.insert(key) {
                edges.push(edge);
            }
        }
    }

    store.set_l2_languages(HashSet::new())?;
    Ok(edges)
}

fn load_sources(root: &Path, entries: &[WalkEntry]) -> Result<Vec<(String, String)>, IndexError> {
    let mut sources = Vec::new();
    for entry in entries.iter().filter(|e| !e.is_dir) {
        let relative = entry.relative_path.to_string_lossy().into_owned();
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };
        sources.push((relative, source));
    }
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sources)
}

pub fn module_path_to_file(importer: &str, module: &str) -> Option<String> {
    let importer_path = PathBuf::from(importer);
    let parent = importer_path.parent().unwrap_or(Path::new(""));
    let module_path = module.replace('.', "/");
    let candidate = parent.join(format!("{module_path}.py"));
    if candidate.components().any(|c| c.as_os_str() == "..") {
        return None;
    }
    Some(candidate.to_string_lossy().into_owned())
}
