use graph_indexer::{export_graph_json, fixture_path, graphs_equal, index, index_with_options, IndexOptions, Tier};
use std::collections::BTreeMap;
use std::fs;

#[test]
fn indexes_unknown_extensions_without_error() {
    let path = fixture_path("unknown_extensions");
    assert!(path.is_dir(), "missing fixture: {}", path.display());

    let result = index(&path).expect("any repo must index without error");
    assert!(
        result.file_count >= 3,
        "expected at least 3 files, got {}",
        result.file_count
    );
    assert!(
        result.unknown_extension_count >= 2,
        "expected unknown extensions to be counted"
    );

    let langs: BTreeMap<_, _> = result
        .capabilities
        .languages
        .iter()
        .map(|l| (l.id.as_str(), l.files))
        .collect();
    assert_eq!(langs.get("unknown"), Some(&2));
    assert_eq!(langs.get("text"), Some(&1));
    assert!(
        !langs.contains_key("ignored"),
        "gitignored files must not appear"
    );

    for lang in &result.capabilities.languages {
        assert_eq!(lang.tier, Tier::L0);
    }
}

#[test]
fn respects_gitignore_in_fixture() {
    use graph_indexer::walk_repo;

    let path = fixture_path("unknown_extensions");
    let result = index(&path).expect("index");
    let total: usize = result.capabilities.languages.iter().map(|l| l.files).sum();
    assert_eq!(
        total, result.file_count,
        "capability file counts should sum to file_count"
    );

    let files: Vec<String> = walk_repo(&path)
        .expect("walk")
        .into_iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.relative_path.to_string_lossy().into_owned())
        .collect();

    assert!(
        !files.iter().any(|p| p.contains("ignored")),
        "gitignored paths must not be indexed: {files:?}"
    );
}

#[test]
fn indexes_universal_mixed_polyglot() {
    let path = fixture_path("universal_mixed");
    let result = index(&path).expect("polyglot repo must index");

    let ids: BTreeMap<_, _> = result
        .capabilities
        .languages
        .iter()
        .map(|l| (l.id.as_str(), (l.files, l.tier)))
        .collect();

    assert!(ids.get("python").map(|(f, _)| *f).unwrap_or(0) >= 1);
    assert!(ids.get("typescript").map(|(f, _)| *f).unwrap_or(0) >= 1);
    assert!(ids.get("go").map(|(f, _)| *f).unwrap_or(0) >= 1);
    assert_eq!(ids.get("python").map(|(_, t)| *t), Some(Tier::L1));
    assert_eq!(ids.get("go").map(|(_, t)| *t), Some(Tier::L1));
}

#[test]
fn indexes_ruby_scripts_at_l1() {
    let path = fixture_path("ruby_scripts");
    let result = index(&path).expect("ruby fixture indexes");
    assert!(result.symbol_count >= 1);
    let ruby = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "ruby")
        .expect("ruby capability");
    assert_eq!(ruby.tier, Tier::L1);
}

#[test]
fn persists_to_sqlite_when_database_path_set() {
    let path = fixture_path("python_simple");
    let temp = tempfile::tempdir().expect("tempdir");
    let db_path = temp.path().join("index.db");

    let result = index_with_options(
        &path,
        IndexOptions {
            database_path: Some(db_path.clone()),
        },
    )
    .expect("index with db");

    assert!(db_path.is_file());
    assert!(result.file_count >= 2);
    assert!(result.symbol_count >= 3);

    let store = graph_indexer::GraphStore::open(&db_path).expect("reopen db");
    assert_eq!(store.file_count().expect("count"), result.file_count);
    assert_eq!(store.symbol_count().expect("symbols"), result.symbol_count);
}

#[test]
fn go_service_resolves_calls_at_l2() {
    let path = fixture_path("go_service");
    let result = index(&path).expect("go index");
    let go = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "go")
        .expect("go");
    assert_eq!(go.tier, Tier::L2);
}

#[test]
fn rust_crate_resolves_calls_at_l2() {
    let path = fixture_path("rust_crate");
    let result = index(&path).expect("rust index");
    let rust = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "rust")
        .expect("rust");
    assert_eq!(rust.tier, Tier::L2);
}

#[test]
fn python_branches_extracts_decision_nodes() {
    let path = fixture_path("python_branches");
    let store = graph_indexer::indexed_store(&path).expect("index branches");

    let branches: Vec<_> = store
        .list_nodes()
        .expect("nodes")
        .into_iter()
        .filter(|n| n.kind == graph_indexer::NodeKind::Branch)
        .collect();
    assert!(!branches.is_empty(), "expected branch nodes");

    let branches_to = store
        .list_edges()
        .expect("edges")
        .into_iter()
        .filter(|e| e.kind == graph_indexer::EdgeKind::BranchesTo)
        .count();
    assert!(branches_to >= 2, "expected BRANCHES_TO edges");
}

#[test]
fn express_routes_handles_handlers() {
    let path = fixture_path("express_routes");
    let store = graph_indexer::indexed_store(&path).expect("index express");

    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles),
        "expected HANDLES edge for app.get"
    );
}

#[test]
fn flutter_trace_links_on_pressed_to_api() {
    let path = fixture_path("flutter_trace");
    let store = graph_indexer::indexed_store(&path).expect("index flutter");

    let ui = store
        .list_nodes()
        .expect("nodes")
        .into_iter()
        .find(|n| n.kind == graph_indexer::NodeKind::UiElement)
        .expect("flutter ui element");
    assert!(ui.name.contains("onPressed"));

    let edges = store.list_edges().expect("edges");
    assert!(edges.iter().any(|e| e.kind == graph_indexer::EdgeKind::Triggers));
    assert!(edges.iter().any(|e| e.kind == graph_indexer::EdgeKind::Fetches));
    assert!(edges.iter().any(|e| e.kind == graph_indexer::EdgeKind::Handles));
}

#[test]
fn flask_routes_handles_health() {
    let path = fixture_path("flask_routes");
    let store = graph_indexer::indexed_store(&path).expect("index flask");
    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles)
    );
}

#[test]
fn django_routes_handles_health() {
    let path = fixture_path("django_routes");
    let store = graph_indexer::indexed_store(&path).expect("index django");
    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles)
    );
}

#[test]
fn actix_routes_handles_health() {
    let path = fixture_path("actix_routes");
    let store = graph_indexer::indexed_store(&path).expect("index actix");
    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles)
    );
}

#[test]
fn react_fastapi_trace_links_ui_to_api() {
    let path = fixture_path("react_fastapi_trace");
    let store = graph_indexer::indexed_store(&path).expect("index react_fastapi");

    let has_triggers = store
        .list_edges()
        .expect("edges")
        .iter()
        .any(|e| e.kind == graph_indexer::EdgeKind::Triggers);
    let has_fetches = store
        .list_edges()
        .expect("edges")
        .iter()
        .any(|e| e.kind == graph_indexer::EdgeKind::Fetches);
    let has_handles = store
        .list_edges()
        .expect("edges")
        .iter()
        .any(|e| e.kind == graph_indexer::EdgeKind::Handles);

    assert!(has_triggers, "expected UI TRIGGERS edge");
    assert!(has_fetches, "expected cross-layer FETCHES edge");
    assert!(has_handles, "expected route HANDLES edge");

    let result = index(&path).expect("capabilities");
    let ts = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "typescript")
        .expect("typescript");
    assert_eq!(ts.tier, Tier::L4);
    let py = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "python")
        .expect("python");
    assert_eq!(py.tier, Tier::L4);
}

#[test]
fn typescript_express_resolves_calls_at_l2() {
    let path = fixture_path("typescript_express");
    let result = index(&path).expect("ts index");
    let ts = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "typescript")
        .expect("typescript");
    assert_eq!(ts.tier, Tier::L2);
}

#[test]
fn java_spring_handles_get_mapping() {
    let path = fixture_path("java_spring");
    let store = graph_indexer::indexed_store(&path).expect("index java");

    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles),
        "expected Spring HANDLES edge"
    );
    let result = index(&path).expect("capabilities");
    let java = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "java")
        .expect("java");
    assert_eq!(java.tier, Tier::L3);
}

#[test]
fn gin_routes_handles_handlers() {
    let path = fixture_path("gin_routes");
    let store = graph_indexer::indexed_store(&path).expect("index gin");

    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Handles),
        "expected Gin HANDLES edge"
    );
}

#[test]
fn php_simple_resolves_calls_at_l2() {
    let path = fixture_path("php_simple");
    let result = index(&path).expect("php index");
    let php = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "php")
        .expect("php");
    assert_eq!(php.tier, Tier::L2);

    let store = graph_indexer::indexed_store(&path).expect("store");
    assert!(
        store
            .list_edges()
            .expect("edges")
            .iter()
            .any(|e| e.kind == graph_indexer::EdgeKind::Calls),
        "expected PHP CALLS edge"
    );
}

#[test]
fn swift_simple_resolves_calls_at_l2() {
    let path = fixture_path("swift_simple");
    let result = index(&path).expect("swift index");
    let swift = result
        .capabilities
        .languages
        .iter()
        .find(|l| l.id == "swift")
        .expect("swift");
    assert_eq!(swift.tier, Tier::L2);
}

#[test]
fn python_simple_matches_golden() {
    let path = fixture_path("python_simple");
    assert!(path.is_dir(), "missing fixture: {}", path.display());

    let golden_path = path.join("expected/graph.json");
    let golden = fs::read_to_string(&golden_path).expect("read golden");
    let expected: serde_json::Value =
        serde_json::from_str(&golden).expect("golden must be valid JSON");

    let actual_json = export_graph_json(&path).expect("export graph");
    let actual: serde_json::Value =
        serde_json::from_str(&actual_json).expect("actual must be valid JSON");

    assert!(
        graphs_equal(&expected, &actual),
        "graph mismatch.\nexpected:\n{}\nactual:\n{}",
        serde_json::to_string_pretty(&expected).unwrap(),
        serde_json::to_string_pretty(&actual).unwrap()
    );

    let functions = actual
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|nodes| {
            nodes
                .iter()
                .filter(|n| n.get("kind").and_then(|k| k.as_str()) == Some("function"))
                .count()
        })
        .unwrap_or(0);
    assert!(
        functions >= 2,
        "expected at least 2 function nodes, got {functions}"
    );
}
