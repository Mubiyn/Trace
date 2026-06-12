//! HTTP responses match the OpenAPI contract shapes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use graph_indexer::fixture_path;
use graph_server::{AppState, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

fn greet_id() -> &'static str {
    "sym:main.py:function:greet:4"
}

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value: Value =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "_raw": String::from_utf8_lossy(&bytes) }));
    (status, value)
}

async fn request(
    app: &mut axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let payload = body
        .map(|b| b.to_string())
        .unwrap_or_default();
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(payload))
        .unwrap();
    body_json(app.oneshot(req).await.expect("response")).await
}

async fn wait_for_index(app: &mut axum::Router, job_id: &str) -> Value {
    for _ in 0..100 {
        let (status, body) = request(app, "GET", &format!("/index/{job_id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        if body["status"] == "complete" || body["status"] == "failed" {
            assert_eq!(body["status"], "complete", "index failed: {body}");
            return body;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("index job did not complete in time");
}

async fn indexed_app() -> axum::Router {
    let state = AppState::ephemeral();
    let mut app = router(state);
    let fixture = fixture_path("python_simple");
    let repo = fixture.display().to_string();
    let (status, body) = request(
        &mut app,
        "POST",
        "/index",
        Some(json!({ "path": repo })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let job_id = body["job_id"].as_str().expect("job_id");
    wait_for_index(&mut app, job_id).await;
    app
}

#[tokio::test]
async fn get_health_matches_schema() {
    let mut app = router(AppState::ephemeral());
    let (status, body) = request(&mut app, "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn post_index_and_poll_status() {
    let mut app = indexed_app().await;
    let fixture = fixture_path("python_simple");
    let (status, body) = request(
        &mut app,
        "GET",
        &format!(
            "/capabilities?repo={}",
            urlencoding::encode(&fixture.display().to_string())
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["capabilities"]["languages"].is_array());
    assert!(body["file_count"].as_u64().unwrap_or(0) > 0);
}

#[tokio::test]
async fn post_query_callers_matches_schema() {
    let mut app = indexed_app().await;
    let (status, body) = request(
        &mut app,
        "POST",
        "/query",
        Some(json!({ "op": "callers", "id": "sym:utils.py:function:format_message:1" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let neighbors = body["neighbors"].as_array().expect("neighbors array");
    assert!(!neighbors.is_empty());
    assert!(neighbors[0]["node"]["name"].is_string());
    assert!(neighbors[0]["edge"]["kind"].is_string());
}

#[tokio::test]
async fn post_query_search_matches_schema() {
    let mut app = indexed_app().await;
    let (status, body) = request(
        &mut app,
        "POST",
        "/query",
        Some(json!({ "op": "search", "query": "greet", "limit": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let nodes = body["nodes"].as_array().expect("nodes");
    assert!(nodes.iter().any(|n| n["name"] == "greet"));
}

#[tokio::test]
async fn post_query_trace_matches_schema() {
    let mut app = indexed_app().await;
    let (status, body) = request(
        &mut app,
        "POST",
        "/query",
        Some(json!({ "op": "trace", "id": greet_id(), "depth": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let hops = body["hops"].as_array().expect("hops");
    assert!(hops.len() >= 2);
    assert_eq!(hops[0]["node"]["name"], "greet");
}

#[tokio::test]
async fn query_without_index_returns_conflict() {
    let mut app = router(AppState::ephemeral());
    let (status, body) = request(
        &mut app,
        "POST",
        "/query",
        Some(json!({ "op": "search", "query": "greet" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "not_indexed");
}
