use graph_engine::{BoundaryDirection, GraphEngine};
use graph_indexer::fixture_path;

fn engine() -> GraphEngine {
    GraphEngine::index(&fixture_path("python_simple")).expect("index python_simple")
}

#[test]
fn search_finds_greet() {
    let engine = engine();
    let hits = engine.search("greet", 10).expect("search");
    assert!(hits.iter().any(|n| n.name == "greet"));
}

#[test]
fn callees_of_greet() {
    let engine = engine();
    let greet_id = "sym:main.py:function:greet:4";
    let callees = engine.callees(greet_id).expect("callees");
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].node.name, "format_message");
    assert_eq!(callees[0].edge.kind, graph_indexer::EdgeKind::Calls);
}

#[test]
fn callers_of_format_message() {
    let engine = engine();
    let target = "sym:utils.py:function:format_message:1";
    let callers = engine.callers(target).expect("callers");
    assert_eq!(callers.len(), 1);
    assert_eq!(callers[0].node.name, "greet");
}

#[test]
fn impact_includes_greet() {
    let engine = engine();
    let target = "sym:utils.py:function:format_message:1";
    let impact = engine.impact(target, 2).expect("impact");
    assert!(impact.iter().any(|n| n.name == "greet"));
}

#[test]
fn trace_follows_call_chain() {
    let engine = engine();
    let root = "sym:main.py:function:greet:4";
    let trace = engine.trace(root, 5).expect("trace");
    assert!(trace.hops.len() >= 2);
    assert_eq!(trace.hops[0].node.name, "greet");
    assert_eq!(trace.hops[1].node.name, "format_message");
}

#[test]
fn entry_points_include_greet() {
    let engine = engine();
    let roots = engine.entry_points().expect("entry_points");
    assert!(roots.iter().any(|n| n.name == "greet"));
}

#[test]
fn subgraph_utils_has_boundary_ghost() {
    let engine = engine();
    let sub = engine.subgraph("utils.py", true).expect("subgraph");
    assert!(sub.nodes.iter().all(|n| {
        n.relative_path == "utils.py" || n.parent_file.as_deref() == Some("utils.py")
    }));
    assert!(
        sub.ghosts
            .iter()
            .any(|g| g.direction == BoundaryDirection::Ingress && g.name == "greet")
    );
}

#[test]
fn search_respects_limit() {
    let engine = engine();
    let hits = engine.search("py", 1).expect("search");
    assert_eq!(hits.len(), 1);
}
