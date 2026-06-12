use graph_engine::GraphEngine;
use graph_indexer::{EdgeKind, NodeKind, fixture_path};

#[test]
fn trace_from_ui_button_reaches_api_handler() {
    let engine = GraphEngine::index(&fixture_path("react_fastapi_trace")).expect("index");
    let ui = engine
        .store()
        .list_nodes()
        .expect("nodes")
        .into_iter()
        .find(|n| n.kind == NodeKind::UiElement)
        .expect("ui element");

    let trace = engine.trace(&ui.id, 6).expect("trace");
    let names: Vec<String> = trace.hops.iter().map(|h| h.node.name.clone()).collect();

    assert!(names.iter().any(|n| n.contains("onClick")));
    assert!(names.iter().any(|n| n == "placeCall"));
    assert!(
        trace.hops.iter().any(|h| {
            h.siblings.iter().any(|s| s.name == "placeCall") || h.node.name == "placeCall"
        })
    );

    let edges = engine.store().list_edges().expect("edges");
    assert!(edges.iter().any(|e| e.kind == EdgeKind::Fetches));
    assert!(edges.iter().any(|e| e.kind == EdgeKind::Handles));
}
