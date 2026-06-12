//! Indexes any repository into a language-tagged graph (L0+).

mod cfg;
mod export;
mod extract;
mod framework;
mod language;
mod model;
mod resolve;
mod store;
mod walk;

pub use export::{count_symbols, graphs_equal};
pub use language::detect_language;
pub use model::{
    symbol_id, Capabilities, EdgeKind, GraphEdge, GraphNode, LanguageCapability, NodeKind, Tier,
};
pub use store::GraphStore;
pub use walk::walk_repo;

use extract::extract_file;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexResult {
    pub file_count: usize,
    pub unknown_extension_count: usize,
    pub symbol_count: usize,
    pub capabilities: Capabilities,
    pub database_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// When set, persist the index to this SQLite file. Otherwise in-memory only.
    pub database_path: Option<PathBuf>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            database_path: None,
        }
    }
}

impl IndexOptions {
    pub fn with_repo_default_db(repo_root: &Path) -> Self {
        Self {
            database_path: Some(repo_root.join(".graph/index.db")),
        }
    }
}

#[derive(Debug)]
pub enum IndexError {
    PathNotFound(String),
    Io(std::io::Error),
    Sqlite(String),
    Export(String),
    Parse(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::PathNotFound(p) => write!(f, "path does not exist: {p}"),
            IndexError::Io(e) => write!(f, "io error: {e}"),
            IndexError::Sqlite(e) => write!(f, "sqlite error: {e}"),
            IndexError::Export(e) => write!(f, "export error: {e}"),
            IndexError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<std::io::Error> for IndexError {
    fn from(value: std::io::Error) -> Self {
        IndexError::Io(value)
    }
}

impl From<ignore::Error> for IndexError {
    fn from(value: ignore::Error) -> Self {
        IndexError::Io(std::io::Error::other(value.to_string()))
    }
}

/// Index a repository at `path`. L0 file walk + L1 symbols where extractors exist.
pub fn index(path: &Path) -> Result<IndexResult, IndexError> {
    index_with_options(path, IndexOptions::default())
}

/// Index with explicit options (database location, etc.).
pub fn index_with_options(path: &Path, options: IndexOptions) -> Result<IndexResult, IndexError> {
    let store = build_index_store(path, &options)?;
    finalize_index_result(&store, options.database_path)
}

/// Index and return canonical graph JSON (for tests and API).
pub fn export_graph_json(path: &Path) -> Result<String, IndexError> {
    let store = build_index_store(path, &IndexOptions::default())?;
    store.export_graph_json()
}

/// Index a repository and return the populated in-memory store (for query engine).
pub fn indexed_store(path: &Path) -> Result<GraphStore, IndexError> {
    build_index_store(path, &IndexOptions::default())
}

pub fn build_index_store(path: &Path, options: &IndexOptions) -> Result<GraphStore, IndexError> {
    if !path.exists() {
        return Err(IndexError::PathNotFound(path.display().to_string()));
    }

    let root = path
        .canonicalize()
        .map_err(|e| IndexError::PathNotFound(format!("{}: {e}", path.display())))?;

    let entries = walk_repo(path)?;
    let store = match &options.database_path {
        Some(db_path) => GraphStore::open(db_path)?,
        None => GraphStore::open_in_memory()?,
    };

    store.insert_walk_entries(&entries, |entry| detect_language(&entry.relative_path))?;

    let mut all_symbols = Vec::new();
    for entry in entries.iter().filter(|e| !e.is_dir) {
        let lang = detect_language(&entry.relative_path);
        let full_path = root.join(&entry.relative_path);
        let Ok(source) = std::fs::read_to_string(&full_path) else {
            continue;
        };
        let relative = entry.relative_path.to_string_lossy().into_owned();
        all_symbols.extend(extract_file(lang, &source, &relative));
    }
    store.insert_symbols(&all_symbols)?;

    let relations = resolve::resolve_relations(&store, &root, &entries)?;
    store.insert_relations(&relations)?;

    let framework_edges = framework::resolve_framework(&store, &root, &entries)?;
    store.insert_relations(&framework_edges)?;

    let cfg_edges = cfg::resolve_cfg(&store, &root, &entries)?;
    store.insert_relations(&cfg_edges)?;

    Ok(store)
}

fn finalize_index_result(
    store: &GraphStore,
    database_path: Option<PathBuf>,
) -> Result<IndexResult, IndexError> {
    Ok(IndexResult {
        file_count: store.file_count()?,
        unknown_extension_count: store
            .list_files()?
            .iter()
            .filter(|f| f.language_id == "unknown")
            .count(),
        symbol_count: store.symbol_count()?,
        capabilities: store.capabilities()?,
        database_path,
    })
}

/// Path to a fixture directory relative to the workspace `graph/` root.
pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_fixture_in_memory() {
        let path = fixture_path("unknown_extensions");
        let result = index(&path).expect("fixture indexes");
        assert!(result.file_count >= 3);
        assert!(result.unknown_extension_count >= 2);
        assert_eq!(result.symbol_count, 0);
    }

    #[test]
    fn python_simple_has_symbols() {
        let path = fixture_path("python_simple");
        let result = index(&path).expect("index");
        assert!(result.symbol_count >= 3);
        let python = result
            .capabilities
            .languages
            .iter()
            .find(|l| l.id == "python")
            .expect("python capability");
        assert_eq!(python.tier, Tier::L2);
    }
}
