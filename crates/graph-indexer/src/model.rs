use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Tier {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::L0 => "L0",
            Tier::L1 => "L1",
            Tier::L2 => "L2",
            Tier::L3 => "L3",
            Tier::L4 => "L4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Directory,
    File,
    Function,
    Class,
    Import,
    /// L3 — UI control wired to a handler (e.g. React `onClick`).
    UiElement,
    /// L3 — HTTP route entry (e.g. FastAPI `@app.post("/path")`).
    Route,
    /// Decision point inside a function (`if`, `try`, etc.).
    Branch,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Directory => "directory",
            NodeKind::File => "file",
            NodeKind::Function => "function",
            NodeKind::Class => "class",
            NodeKind::Import => "import",
            NodeKind::UiElement => "ui_element",
            NodeKind::Route => "route",
            NodeKind::Branch => "branch",
        }
    }

    pub fn is_symbol(self) -> bool {
        matches!(
            self,
            NodeKind::Function
                | NodeKind::Class
                | NodeKind::Import
                | NodeKind::UiElement
                | NodeKind::Route
                | NodeKind::Branch
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    /// L3 — UI element triggers a handler function.
    Triggers,
    /// L3 — Route dispatches to a handler function.
    Handles,
    /// L4 — Client call targets a server route (cross-layer).
    Fetches,
    /// CFG — decision branch may reach an outcome (call target).
    BranchesTo,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Contains => "CONTAINS",
            EdgeKind::Imports => "IMPORTS",
            EdgeKind::Calls => "CALLS",
            EdgeKind::Triggers => "TRIGGERS",
            EdgeKind::Handles => "HANDLES",
            EdgeKind::Fetches => "FETCHES",
            EdgeKind::BranchesTo => "BRANCHES_TO",
        }
    }

    pub fn is_semantic(self) -> bool {
        matches!(
            self,
            EdgeKind::Imports
                | EdgeKind::Calls
                | EdgeKind::Triggers
                | EdgeKind::Handles
                | EdgeKind::Fetches
                | EdgeKind::BranchesTo
        )
    }

    pub fn is_trace(self) -> bool {
        matches!(
            self,
            EdgeKind::Calls
                | EdgeKind::Triggers
                | EdgeKind::Handles
                | EdgeKind::Fetches
                | EdgeKind::BranchesTo
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub relative_path: String,
    pub parent_file: Option<String>,
    pub line: Option<u32>,
    pub extension: Option<String>,
    pub language_id: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub kind: EdgeKind,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedEdge {
    pub from_id: String,
    pub to_id: String,
    pub kind: EdgeKind,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageCapability {
    pub id: String,
    pub files: usize,
    pub tier: Tier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    pub languages: Vec<LanguageCapability>,
}

#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub relative_path: PathBuf,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSymbol {
    pub kind: NodeKind,
    pub name: String,
    pub parent_file: String,
    pub line: u32,
    pub language_id: String,
}

pub fn node_id_for_path(relative_path: &str) -> String {
    if relative_path == "." || relative_path.is_empty() {
        "dir:.".to_string()
    } else if relative_path.starts_with("dir:")
        || relative_path.starts_with("file:")
        || relative_path.starts_with("sym:")
    {
        relative_path.to_string()
    } else {
        format!("path:{relative_path}")
    }
}

pub fn symbol_id(parent_file: &str, kind: NodeKind, name: &str, line: u32) -> String {
    format!(
        "sym:{parent_file}:{}:{name}:{line}",
        kind.as_str()
    )
}

pub fn edge_id(from_id: &str, to_id: &str, kind: EdgeKind) -> String {
    format!("{}:{}:{}", kind.as_str(), from_id, to_id)
}
