use axum::body::Body;
use axum::http::{Request, StatusCode};
use graph_indexer::fixture_path;
use graph_server::{router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::fs;
use tower::ServiceExt;

async fn body_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, value)
}

async fn indexed_app() -> axum::Router {
    let state = AppState::ephemeral();
    state
        .index_path(&fixture_path("python_simple"))
        .await
        .expect("index");
    router(state)
}

#[tokio::test]
async fn post_overlay_enriches_subgraph() {
    let app = indexed_app().await;
    let observed = fs::read_to_string(fixture_path("runtime_overlay").join("observed.json"))
        .expect("read observed");
    let observed: Value = serde_json::from_str(&observed).expect("json");

    let req = Request::builder()
        .method("POST")
        .uri("/overlay")
        .header("content-type", "application/json")
        .body(Body::from(observed.to_string()))
        .unwrap();
    let (status, body) = body_json(app.clone().oneshot(req).await.expect("overlay")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let req = Request::builder()
        .method("POST")
        .uri("/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "op": "subgraph", "scope": ".", "boundary": false }).to_string(),
        ))
        .unwrap();
    let (status, subgraph) = body_json(app.oneshot(req).await.expect("subgraph")).await;
    assert_eq!(status, StatusCode::OK);
    let overlay = subgraph["overlay"].as_object().expect("overlay");
    assert!(overlay["observedNodeIds"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn post_otel_overlay_auto_detected() {
    let app = indexed_app().await;
    let observed = fs::read_to_string(
        graph_indexer::fixture_path("otel_trace").join("export.json"),
    )
    .expect("read otel");
    let observed: Value = serde_json::from_str(&observed).expect("json");

    let req = Request::builder()
        .method("POST")
        .uri("/overlay")
        .header("content-type", "application/json")
        .body(Body::from(observed.to_string()))
        .unwrap();
    let (status, body) = body_json(app.clone().oneshot(req).await.expect("overlay")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["nodeHitCount"].as_u64().unwrap_or(0) >= 2);
}
