//! Graph HTTP API and shared query dispatch for MCP.

mod hosted;
mod mcp;

pub use hosted::{
    default_hosted_dir, ActivateRepoRequest, HostedRepoSummary, ReposListResponse,
};
pub use mcp::{call_mcp_tool, call_mcp_tool_on_state, mcp_tool_names, run_mcp_stdio, McpToolError};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use graph_engine::{
    attach_overlay, parse_runtime_overlay, GraphEngine, Neighbor, RuntimeOverlay, Subgraph,
    TraceResult, ENGINE_VERSION,
};
use graph_indexer::{Capabilities, GraphNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

pub const DEFAULT_ADDR: &str = "127.0.0.1:9847";

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<ServerState>>,
}

struct ServerState {
    active: Option<ActiveIndex>,
    overlay: Option<RuntimeOverlay>,
    hosted: Option<hosted::HostedStore>,
    jobs: HashMap<Uuid, IndexJob>,
}

pub(crate) struct ActiveIndex {
    #[allow(dead_code)]
    pub repo_id: String,
    pub path: PathBuf,
    pub engine: GraphEngine,
    pub capabilities: Capabilities,
    pub file_count: usize,
    pub indexed_at: u64,
    #[allow(dead_code)]
    pub git_url: Option<String>,
    #[allow(dead_code)]
    pub git_ref: Option<String>,
}

struct IndexJob {
    request: IndexRequest,
    status: JobStatus,
    error: Option<String>,
    file_count: Option<usize>,
}

enum JobUpdate {
    Complete { active: Box<ActiveIndex> },
    Failed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexRequest {
    pub path: String,
    #[serde(default)]
    pub repo_id: Option<String>,
    #[serde(default)]
    pub git_url: Option<String>,
    #[serde(default)]
    pub git_ref: Option<String>,
    /// When true (default if hosted dir is configured), persist index for multi-repo registry.
    #[serde(default)]
    pub persist: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct IndexJobResponse {
    pub job_id: Uuid,
    pub status: JobStatus,
}

#[derive(Debug, Serialize)]
pub struct IndexStatusResponse {
    pub job_id: Uuid,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilitiesQuery {
    pub repo: String,
}

#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    pub repo: String,
    pub file_count: usize,
    pub capabilities: Capabilities,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRequest {
    pub op: QueryOp,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub depth: Option<usize>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub boundary: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueryOp {
    Search,
    Callers,
    Callees,
    Trace,
    Subgraph,
    Impact,
    EntryPoints,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum QueryResponse {
    Nodes { nodes: Vec<GraphNode> },
    Neighbors { neighbors: Vec<Neighbor> },
    Trace(TraceResult),
    Subgraph(Subgraph),
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: String,
    pub code: &'static str,
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    NotIndexed,
    RepoMismatch,
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::NotIndexed => (
                StatusCode::CONFLICT,
                "not_indexed",
                "no repository indexed; POST /index first".to_string(),
            ),
            ApiError::RepoMismatch => (
                StatusCode::NOT_FOUND,
                "repo_not_indexed",
                "requested repo is not the active index".to_string(),
            ),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", msg),
        };
        (
            status,
            Json(ApiErrorBody {
                error: message,
                code,
            }),
        )
            .into_response()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::with_hosted_dir(hosted::default_hosted_dir())
    }

    pub fn with_hosted_dir(hosted_dir: Option<PathBuf>) -> Self {
        let hosted = hosted_dir.and_then(|dir| hosted::HostedStore::open(dir).ok());
        let active = hosted.as_ref().and_then(|h| h.try_restore_active());
        Self {
            inner: Arc::new(Mutex::new(ServerState {
                active,
                overlay: None,
                hosted,
                jobs: HashMap::new(),
            })),
        }
    }

    /// Ephemeral in-memory index only (tests).
    pub fn ephemeral() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ServerState {
                active: None,
                overlay: None,
                hosted: None,
                jobs: HashMap::new(),
            })),
        }
    }

    /// Index synchronously — used in tests and MCP bootstrap.
    pub async fn index_path(&self, path: &FsPath) -> Result<(), ApiError> {
        let canonical = canonical_repo_path(path)?;
        let state = self.clone();
        tokio::task::spawn_blocking(move || state.index_path_blocking(&canonical))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    }

    fn index_path_blocking(&self, path: &FsPath) -> Result<(), ApiError> {
        let req = IndexRequest {
            path: path.display().to_string(),
            repo_id: None,
            git_url: None,
            git_ref: None,
            persist: Some(false),
        };
        let active = self
            .index_request_blocking(&req)
            .map_err(ApiError::Internal)?;
        let mut guard = self.inner.lock().unwrap();
        guard.active = Some(active);
        guard.overlay = None;
        Ok(())
    }

    fn index_request_blocking(&self, req: &IndexRequest) -> Result<ActiveIndex, String> {
        let path = resolve_index_path(req).map_err(|e| format!("{e:?}"))?;
        let repo_id = req
            .repo_id
            .clone()
            .unwrap_or_else(|| hosted::default_repo_id(&path));

        let use_hosted = {
            let guard = self.inner.lock().unwrap();
            req.persist.unwrap_or(guard.hosted.is_some())
        };

        if use_hosted {
            let mut guard = self.inner.lock().unwrap();
            let hosted = guard
                .hosted
                .as_mut()
                .ok_or_else(|| "persist requested but hosted store not configured".to_string())?;
            return hosted
                .persist_index(
                    &repo_id,
                    &path,
                    req.git_url.clone(),
                    req.git_ref.clone(),
                )
                .map_err(|e| format!("{e:?}"));
        }

        build_active_index_blocking(&path, &repo_id, req.git_url.clone(), req.git_ref.clone())
    }

    pub fn dispatch_query_sync(&self, req: QueryRequest) -> Result<QueryResponse, ApiError> {
        let guard = self.inner.lock().unwrap();
        let active = guard.active.as_ref().ok_or(ApiError::NotIndexed)?;
        let overlay = guard.overlay.clone();
        let mut resp = dispatch_query_on_engine(&active.engine, req)?;
        if let (Some(overlay), QueryResponse::Subgraph(subgraph)) = (overlay, &mut resp) {
            *subgraph = attach_overlay(subgraph.clone(), &overlay);
        }
        Ok(resp)
    }

    pub fn set_overlay_sync(&self, body: serde_json::Value) -> Result<OverlayStatusResponse, ApiError> {
        let mut guard = self.inner.lock().unwrap();
        let active = guard.active.as_ref().ok_or(ApiError::NotIndexed)?;
        let overlay = parse_runtime_overlay(&body, active.engine.store())
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        let node_count = overlay.node_hits.len() + overlay.paths.len();
        guard.overlay = Some(overlay);
        Ok(OverlayStatusResponse {
            status: "ok",
            path_count: guard.overlay.as_ref().map(|o| o.paths.len()).unwrap_or(0),
            node_hit_count: guard.overlay.as_ref().map(|o| o.node_hits.len()).unwrap_or(0),
            observation_count: node_count,
        })
    }

    pub fn overlay_status_sync(&self) -> Result<Option<RuntimeOverlay>, ApiError> {
        let guard = self.inner.lock().unwrap();
        if guard.active.is_none() {
            return Err(ApiError::NotIndexed);
        }
        Ok(guard.overlay.clone())
    }

    pub fn clear_overlay_sync(&self) -> Result<(), ApiError> {
        let mut guard = self.inner.lock().unwrap();
        if guard.active.is_none() {
            return Err(ApiError::NotIndexed);
        }
        guard.overlay = None;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayStatusResponse {
    pub status: &'static str,
    pub path_count: usize,
    pub node_hit_count: usize,
    pub observation_count: usize,
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/index", post(start_index))
        .route("/index/{job_id}", get(index_status))
        .route("/capabilities", get(capabilities))
        .route("/query", post(query))
        .route("/overlay", post(set_overlay).get(get_overlay).delete(clear_overlay))
        .route("/repos", get(list_repos))
        .route("/activate", post(activate_repo))
        .layer(cors)
        .with_state(state)
}

pub fn dispatch_query_on_engine(
    engine: &GraphEngine,
    req: QueryRequest,
) -> Result<QueryResponse, ApiError> {
    match req.op {
        QueryOp::Search => {
            let q = req
                .query
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ApiError::BadRequest("search requires query".into()))?;
            let limit = req.limit.unwrap_or(20);
            let nodes = engine.search(&q, limit).map_err(map_engine_error)?;
            Ok(QueryResponse::Nodes { nodes })
        }
        QueryOp::Callers => {
            let id = required_id(&req)?;
            let neighbors = engine.callers(&id).map_err(map_engine_error)?;
            Ok(QueryResponse::Neighbors { neighbors })
        }
        QueryOp::Callees => {
            let id = required_id(&req)?;
            let neighbors = engine.callees(&id).map_err(map_engine_error)?;
            Ok(QueryResponse::Neighbors { neighbors })
        }
        QueryOp::Trace => {
            let id = required_id(&req)?;
            let depth = req.depth.unwrap_or(8);
            let trace = engine.trace(&id, depth).map_err(map_engine_error)?;
            Ok(QueryResponse::Trace(trace))
        }
        QueryOp::Subgraph => {
            let scope = req.scope.unwrap_or_else(|| ".".into());
            let boundary = req.boundary.unwrap_or(true);
            let subgraph = engine
                .subgraph(&scope, boundary)
                .map_err(map_engine_error)?;
            Ok(QueryResponse::Subgraph(subgraph))
        }
        QueryOp::Impact => {
            let id = required_id(&req)?;
            let depth = req.depth.unwrap_or(3);
            let nodes = engine.impact(&id, depth).map_err(map_engine_error)?;
            Ok(QueryResponse::Nodes { nodes })
        }
        QueryOp::EntryPoints => {
            let nodes = engine.entry_points().map_err(map_engine_error)?;
            Ok(QueryResponse::Nodes { nodes })
        }
    }
}

fn required_id(req: &QueryRequest) -> Result<String, ApiError> {
    req.id
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("query requires id".into()))
}

fn map_engine_error(err: graph_engine::EngineError) -> ApiError {
    match err {
        graph_engine::EngineError::NotFound(id) => ApiError::NotFound(id),
        graph_engine::EngineError::Indexer(e) => ApiError::Internal(e.to_string()),
    }
}

pub fn canonical_repo_path(path: &FsPath) -> Result<PathBuf, ApiError> {
    FsPath::new(path)
        .canonicalize()
        .map_err(|e| ApiError::BadRequest(format!("invalid path: {e}")))
}

fn resolve_index_path(req: &IndexRequest) -> Result<PathBuf, ApiError> {
    if let Some(url) = &req.git_url {
        return hosted::clone_git_repo(url, req.git_ref.as_deref());
    }
    canonical_repo_path(FsPath::new(&req.path))
}

fn build_active_index_blocking(
    path: &FsPath,
    repo_id: &str,
    git_url: Option<String>,
    git_ref: Option<String>,
) -> Result<ActiveIndex, String> {
    let engine = GraphEngine::index(path).map_err(|e| e.to_string())?;
    let capabilities = engine.capabilities().map_err(|e| e.to_string())?;
    let file_count = engine
        .store()
        .list_nodes()
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|n| n.kind == graph_indexer::NodeKind::File)
        .count();
    Ok(ActiveIndex {
        repo_id: repo_id.to_string(),
        path: path.to_path_buf(),
        engine,
        capabilities,
        file_count,
        indexed_at: unix_now(),
        git_url,
        git_ref,
    })
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: ENGINE_VERSION,
    })
}

async fn start_index(
    State(state): State<AppState>,
    Json(body): Json<IndexRequest>,
) -> Result<(StatusCode, Json<IndexJobResponse>), ApiError> {
    if body.git_url.is_none() {
        let _ = resolve_index_path(&body)?;
    }
    let job_id = Uuid::new_v4();

    {
        let mut guard = state.inner.lock().unwrap();
        guard.jobs.insert(
            job_id,
            IndexJob {
                request: body.clone(),
                status: JobStatus::Queued,
                error: None,
                file_count: None,
            },
        );
    }

    let state_clone = state.clone();
    let request = body;
    std::thread::spawn(move || {
        {
            let mut guard = state_clone.inner.lock().unwrap();
            if let Some(job) = guard.jobs.get_mut(&job_id) {
                job.status = JobStatus::Running;
            }
        }

        let update = match state_clone.index_request_blocking(&request) {
            Ok(active) => JobUpdate::Complete {
                active: Box::new(active),
            },
            Err(message) => JobUpdate::Failed { message },
        };

        let mut guard = state_clone.inner.lock().unwrap();
        match update {
            JobUpdate::Complete { active } => {
                if let Some(job) = guard.jobs.get_mut(&job_id) {
                    job.status = JobStatus::Complete;
                    job.file_count = Some(active.file_count);
                }
                guard.active = Some(*active);
                guard.overlay = None;
            }
            JobUpdate::Failed { message } => {
                if let Some(job) = guard.jobs.get_mut(&job_id) {
                    job.status = JobStatus::Failed;
                    job.error = Some(message);
                }
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(IndexJobResponse {
            job_id,
            status: JobStatus::Queued,
        }),
    ))
}

async fn index_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<IndexStatusResponse>, ApiError> {
    let guard = state.inner.lock().unwrap();
    let job = guard
        .jobs
        .get(&job_id)
        .ok_or_else(|| ApiError::NotFound(format!("job {job_id}")))?;

    Ok(Json(IndexStatusResponse {
        job_id,
        status: job.status,
        path: Some(job.request.path.clone()),
        error: job.error.clone(),
        file_count: job.file_count,
    }))
}

async fn capabilities(
    State(state): State<AppState>,
    Query(params): Query<CapabilitiesQuery>,
) -> Result<Json<CapabilitiesResponse>, ApiError> {
    let repo = canonical_repo_path(FsPath::new(&params.repo))?;
    let guard = state.inner.lock().unwrap();
    let active = guard.active.as_ref().ok_or(ApiError::NotIndexed)?;
    if active.path != repo {
        return Err(ApiError::RepoMismatch);
    }

    Ok(Json(CapabilitiesResponse {
        repo: active.path.display().to_string(),
        file_count: active.file_count,
        capabilities: active.capabilities.clone(),
    }))
}

async fn query(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || state.dispatch_query_sync(body))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map(Json)
}

async fn list_repos(State(state): State<AppState>) -> Result<Json<ReposListResponse>, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let guard = state.inner.lock().unwrap();
        let hosted = guard
            .hosted
            .as_ref()
            .ok_or(ApiError::BadRequest("hosted registry not enabled".into()))?;
        Ok(Json(hosted.list_summaries()))
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
}

async fn activate_repo(
    State(state): State<AppState>,
    Json(body): Json<ActivateRepoRequest>,
) -> Result<Json<HostedRepoSummary>, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = state.inner.lock().unwrap();
        let hosted = guard
            .hosted
            .as_mut()
            .ok_or(ApiError::BadRequest("hosted registry not enabled".into()))?;
        let repo_id = body.repo_id.clone();
        let active = hosted.activate(&repo_id)?;
        guard.active = Some(active);
        guard.overlay = None;
        Ok(Json(HostedRepoSummary {
            repo_id,
            path: guard
                .active
                .as_ref()
                .map(|a| a.path.display().to_string())
                .unwrap_or_default(),
            file_count: guard.active.as_ref().map(|a| a.file_count).unwrap_or(0),
            indexed_at: guard.active.as_ref().map(|a| a.indexed_at).unwrap_or(0),
            active: true,
        }))
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
}

async fn set_overlay(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<OverlayStatusResponse>, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || state.set_overlay_sync(body))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map(Json)
}

async fn get_overlay(
    State(state): State<AppState>,
) -> Result<Json<Option<RuntimeOverlay>>, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || state.overlay_status_sync())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map(Json)
}

async fn clear_overlay(State(state): State<AppState>) -> Result<StatusCode, ApiError> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || state.clear_overlay_sync())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))??;
    Ok(StatusCode::NO_CONTENT)
}
