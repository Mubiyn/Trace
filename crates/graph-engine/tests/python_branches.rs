use graph_engine::GraphEngine;
use graph_indexer::{EdgeKind, NodeKind, fixture_path};

#[test]
fn trace_from_function_through_branch_to_outcomes() {
    let engine = GraphEngine::index(&fixture_path("python_branches")).expect("index");
    let handle = engine
        .store()
        .list_nodes()
        .expect("nodes")
        .into_iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "handle")
        .expect("handle function");

    let branches_to: Vec<_> = engine
        .store()
        .list_edges()
        .expect("edges")
        .into_iter()
        .filter(|e| e.kind == EdgeKind::BranchesTo)
        .collect();
    assert!(
        branches_to
            .iter()
            .any(|e| e.from_id == handle.id),
        "handle should BRANCHES_TO a decision node: {branches_to:?}"
    );

    let trace = engine.trace(&handle.id, 5).expect("trace");
    let hop_names: Vec<String> = trace.hops.iter().map(|h| h.node.name.clone()).collect();
    let hop_kinds: Vec<_> = trace.hops.iter().map(|h| h.node.kind).collect();
    assert!(
        hop_kinds.contains(&NodeKind::Branch),
        "trace should include branch decision nodes; hops={hop_names:?} kinds={hop_kinds:?}"
    );

    let outcome_names: Vec<String> = trace
        .hops
        .iter()
        .flat_map(|h| {
            std::iter::once(h.node.name.clone()).chain(
                h.siblings
                    .iter()
                    .map(|s| s.name.clone()),
            )
        })
        .collect();
    assert!(outcome_names.iter().any(|n| n == "on_ok"));
    assert!(outcome_names.iter().any(|n| n == "on_denied"));

    let edges = engine.store().list_edges().expect("edges");
    assert!(edges.iter().any(|e| e.kind == EdgeKind::BranchesTo));
}
