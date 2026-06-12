//! MCP tool JSON matches HTTP `POST /query` responses.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use graph_indexer::fixture_path;
use graph_server::{call_mcp_tool_on_state, router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn greet_id() -> &'static str {
    "sym:main.py:function:greet:4"
}

fn format_message_id() -> &'static str {
    "sym:utils.py:function:format_message:1"
}

async fn http_query(app: &mut axum::Router, body: Value) -> Value {
    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

async fn indexed_state_and_router() -> (AppState, axum::Router) {
    let state = AppState::ephemeral();
    state
        .index_path(&fixture_path("python_simple"))
        .await
        .expect("index_path");
    let app = router(state.clone());
    (state, app)
}

#[tokio::test]
async fn mcp_graph_callers_matches_http() {
    let (state, mut app) = indexed_state_and_router().await;
    let http_body = json!({ "op": "callers", "id": format_message_id() });
    let http = http_query(&mut app, http_body).await;

    let mcp = call_mcp_tool_on_state(&state, "graph_callers", json!({ "id": format_message_id() }))
        .expect("mcp callers");
    assert_eq!(mcp, http);
}

#[tokio::test]
async fn mcp_graph_trace_matches_http() {
    let (state, mut app) = indexed_state_and_router().await;
    let http_body = json!({ "op": "trace", "id": greet_id(), "depth": 5 });
    let http = http_query(&mut app, http_body).await;

    let mcp = call_mcp_tool_on_state(
        &state,
        "graph_trace",
        json!({ "id": greet_id(), "depth": 5 }),
    )
    .expect("mcp trace");
    assert_eq!(mcp, http);
}

#[tokio::test]
async fn mcp_graph_search_matches_http() {
    let (state, mut app) = indexed_state_and_router().await;
    let http_body = json!({ "op": "search", "query": "greet", "limit": 5 });
    let http = http_query(&mut app, http_body).await;

    let mcp = call_mcp_tool_on_state(
        &state,
        "graph_search",
        json!({ "query": "greet", "limit": 5 }),
    )
    .expect("mcp search");
    assert_eq!(mcp, http);
}

#[tokio::test]
async fn mcp_graph_entry_points_matches_http() {
    let (state, mut app) = indexed_state_and_router().await;
    let http_body = json!({ "op": "entryPoints" });
    let http = http_query(&mut app, http_body).await;

    let mcp = call_mcp_tool_on_state(&state, "graph_entry_points", json!({}))
        .expect("mcp entry points");
    assert_eq!(mcp, http);
}

#[tokio::test]
async fn mcp_graph_impact_matches_http() {
    let (state, mut app) = indexed_state_and_router().await;
    let http_body = json!({ "op": "impact", "id": format_message_id(), "depth": 3 });
    let http = http_query(&mut app, http_body).await;

    let mcp = call_mcp_tool_on_state(
        &state,
        "graph_impact",
        json!({ "id": format_message_id(), "depth": 3 }),
    )
    .expect("mcp impact");
    assert_eq!(mcp, http);
}
