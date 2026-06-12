use axum::body::Body;
use axum::http::{Request, StatusCode};
use graph_indexer::fixture_path;
use graph_server::{router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
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

async fn request(
    app: &mut axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(payload))
        .unwrap();
    body_json(app.oneshot(req).await.expect("response")).await
}

#[tokio::test]
async fn hosted_registry_persists_and_lists_repos() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state = AppState::with_hosted_dir(Some(dir.path().to_path_buf()));
    let mut app = router(state);

    let fixture = fixture_path("python_simple");
    let (status, body) = request(
        &mut app,
        "POST",
        "/index",
        Some(json!({
            "path": fixture.display().to_string(),
            "repoId": "python_simple",
            "persist": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let job_id = body["jobId"].as_str().or_else(|| body["job_id"].as_str()).expect("job");

    for _ in 0..100 {
        let (status, job) = request(&mut app, "GET", &format!("/index/{job_id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        if job["status"] == "complete" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let (status, repos) = request(&mut app, "GET", "/repos", None).await;
    assert_eq!(status, StatusCode::OK);
    let list = repos["repos"].as_array().expect("repos array");
    assert!(list.iter().any(|r| r["repoId"] == "python_simple" || r["repo_id"] == "python_simple"));

    let restarted = AppState::with_hosted_dir(Some(dir.path().to_path_buf()));
    let mut restarted_app = router(restarted);
    let (status, repos2) = request(&mut restarted_app, "GET", "/repos", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        repos2["repos"]
            .as_array()
            .expect("repos")
            .iter()
            .any(|r| r["active"] == true)
    );
}
