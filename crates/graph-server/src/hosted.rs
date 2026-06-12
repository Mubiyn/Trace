//! Multi-repo hosted index registry (local MVP for [HOSTED_INDEX.md](../../HOSTED_INDEX.md)).

use crate::{ActiveIndex, ApiError};
use graph_engine::GraphEngine;
use graph_indexer::{index_with_options, IndexOptions};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedRegistry {
    pub repos: Vec<HostedRepoRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_repo_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedRepoRecord {
    pub repo_id: String,
    pub path: String,
    pub db_path: String,
    pub indexed_at: u64,
    pub file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReposListResponse {
    pub repos: Vec<HostedRepoSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedRepoSummary {
    pub repo_id: String,
    pub path: String,
    pub file_count: usize,
    pub indexed_at: u64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateRepoRequest {
    pub repo_id: String,
}

pub struct HostedStore {
    pub dir: PathBuf,
    registry: HostedRegistry,
}

impl HostedStore {
    pub fn open(dir: PathBuf) -> Result<Self, ApiError> {
        fs::create_dir_all(&dir).map_err(|e| ApiError::Internal(e.to_string()))?;
        let registry_path = dir.join("registry.json");
        let registry = if registry_path.exists() {
            let raw = fs::read_to_string(&registry_path).map_err(|e| ApiError::Internal(e.to_string()))?;
            serde_json::from_str(&raw).unwrap_or(HostedRegistry {
                repos: Vec::new(),
                active_repo_id: None,
            })
        } else {
            HostedRegistry {
                repos: Vec::new(),
                active_repo_id: None,
            }
        };
        Ok(Self { dir, registry })
    }

    pub fn list_summaries(&self) -> ReposListResponse {
        let active = self.registry.active_repo_id.as_deref();
        ReposListResponse {
            repos: self
                .registry
                .repos
                .iter()
                .map(|r| HostedRepoSummary {
                    repo_id: r.repo_id.clone(),
                    path: r.path.clone(),
                    file_count: r.file_count,
                    indexed_at: r.indexed_at,
                    active: active == Some(r.repo_id.as_str()),
                })
                .collect(),
        }
    }

    pub fn persist_index(
        &mut self,
        repo_id: &str,
        repo_path: &Path,
        git_url: Option<String>,
        git_ref: Option<String>,
    ) -> Result<ActiveIndex, ApiError> {
        let repo_dir = self.dir.join(sanitize_repo_id(repo_id));
        fs::create_dir_all(&repo_dir).map_err(|e| ApiError::Internal(e.to_string()))?;
        let db_path = repo_dir.join("index.db");

        let result = index_with_options(
            repo_path,
            IndexOptions {
                database_path: Some(db_path.clone()),
            },
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        let engine = GraphEngine::open_persisted(&db_path).map_err(|e| ApiError::Internal(e.to_string()))?;
        let capabilities = engine.capabilities().map_err(|e| ApiError::Internal(e.to_string()))?;

        let record = HostedRepoRecord {
            repo_id: repo_id.to_string(),
            path: repo_path.display().to_string(),
            db_path: db_path.display().to_string(),
            indexed_at: unix_now(),
            file_count: result.file_count,
            git_url,
            git_ref,
        };

        self.registry
            .repos
            .retain(|r| r.repo_id != record.repo_id);
        self.registry.repos.push(record.clone());
        self.registry.active_repo_id = Some(record.repo_id.clone());
        self.save_registry()?;

        Ok(ActiveIndex {
            repo_id: record.repo_id,
            path: repo_path.to_path_buf(),
            engine,
            capabilities,
            file_count: result.file_count,
            indexed_at: record.indexed_at,
            git_url: record.git_url.clone(),
            git_ref: record.git_ref.clone(),
        })
    }

    pub fn activate(&mut self, repo_id: &str) -> Result<ActiveIndex, ApiError> {
        let record = self
            .registry
            .repos
            .iter()
            .find(|r| r.repo_id == repo_id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound(format!("repo {repo_id}")))?;

        let engine = GraphEngine::open_persisted(Path::new(&record.db_path))
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        let capabilities = engine.capabilities().map_err(|e| ApiError::Internal(e.to_string()))?;

        self.registry.active_repo_id = Some(repo_id.to_string());
        self.save_registry()?;

        Ok(ActiveIndex {
            repo_id: record.repo_id,
            path: PathBuf::from(&record.path),
            engine,
            capabilities,
            file_count: record.file_count,
            indexed_at: record.indexed_at,
            git_url: record.git_url,
            git_ref: record.git_ref,
        })
    }

    pub fn try_restore_active(&self) -> Option<ActiveIndex> {
        let repo_id = self.registry.active_repo_id.as_ref()?;
        let record = self
            .registry
            .repos
            .iter()
            .find(|r| r.repo_id == *repo_id)?;
        let engine = GraphEngine::open_persisted(Path::new(&record.db_path)).ok()?;
        let capabilities = engine.capabilities().ok()?;
        Some(ActiveIndex {
            repo_id: record.repo_id.clone(),
            path: PathBuf::from(&record.path),
            engine,
            capabilities,
            file_count: record.file_count,
            indexed_at: record.indexed_at,
            git_url: record.git_url.clone(),
            git_ref: record.git_ref.clone(),
        })
    }

    fn save_registry(&self) -> Result<(), ApiError> {
        let path = self.dir.join("registry.json");
        let json = serde_json::to_string_pretty(&self.registry)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        fs::write(path, json).map_err(|e| ApiError::Internal(e.to_string()))
    }
}

pub fn default_hosted_dir() -> Option<PathBuf> {
    std::env::var("GRAPH_HOSTED_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            dirs_home().map(|home| home.join(".graph").join("hosted"))
        })
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

pub fn sanitize_repo_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn default_repo_id(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(sanitize_repo_id)
        .unwrap_or_else(|| "repo".to_string())
}

pub fn clone_git_repo(url: &str, git_ref: Option<&str>) -> Result<PathBuf, ApiError> {
    let tmp = tempfile::tempdir().map_err(|e| ApiError::Internal(e.to_string()))?;
    let dest = tmp.path().join("repo");
    let mut clone = Command::new("git");
    clone.args(["clone", "--depth", "1"]);
    if let Some(reference) = git_ref {
        clone.args(["--branch", reference]);
    }
    clone.arg(url).arg(&dest);
    let status = clone.status().map_err(|e| ApiError::BadRequest(format!("git not available: {e}")))?;
    if !status.success() {
        return Err(ApiError::BadRequest(format!("git clone failed for {url}")));
    }
    // Leak tempdir by converting to path — caller indexes synchronously in same process.
    let path = dest.clone();
    std::mem::forget(tmp);
    Ok(path)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
