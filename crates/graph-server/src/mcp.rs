//! MCP tool dispatch and stdio JSON-RPC server.

use crate::{
    ApiError, AppState, QueryOp, QueryRequest, dispatch_query_on_engine,
};
use graph_engine::GraphEngine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "graph-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const TOOL_GRAPH_CALLERS: &str = "graph_callers";
pub const TOOL_GRAPH_TRACE: &str = "graph_trace";
pub const TOOL_GRAPH_SEARCH: &str = "graph_search";
pub const TOOL_GRAPH_ENTRY_POINTS: &str = "graph_entry_points";
pub const TOOL_GRAPH_IMPACT: &str = "graph_impact";

pub fn mcp_tool_names() -> &'static [&'static str] {
    &[
        TOOL_GRAPH_CALLERS,
        TOOL_GRAPH_TRACE,
        TOOL_GRAPH_SEARCH,
        TOOL_GRAPH_ENTRY_POINTS,
        TOOL_GRAPH_IMPACT,
    ]
}

#[derive(Debug)]
pub enum McpToolError {
    Api(ApiError),
    UnknownTool(String),
    BadArgs(String),
}

impl From<ApiError> for McpToolError {
    fn from(value: ApiError) -> Self {
        McpToolError::Api(value)
    }
}

impl std::fmt::Display for McpToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpToolError::Api(e) => write!(f, "{e:?}"),
            McpToolError::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            McpToolError::BadArgs(msg) => write!(f, "{msg}"),
        }
    }
}

/// Run an MCP tool and return the same JSON shape as `POST /query`.
fn query_response_as_json_value(resp: crate::QueryResponse) -> Result<Value, McpToolError> {
    let wire = serde_json::to_string(&resp).map_err(|e| McpToolError::BadArgs(e.to_string()))?;
    serde_json::from_str(&wire).map_err(|e| McpToolError::BadArgs(e.to_string()))
}

pub fn call_mcp_tool(engine: &GraphEngine, tool: &str, args: Value) -> Result<Value, McpToolError> {
    let req = tool_to_query(tool, args)?;
    let resp = dispatch_query_on_engine(engine, req)?;
    query_response_as_json_value(resp)
}

pub fn call_mcp_tool_on_state(
    state: &AppState,
    tool: &str,
    args: Value,
) -> Result<Value, McpToolError> {
    let req = tool_to_query(tool, args)?;
    let resp = state.dispatch_query_sync(req)?;
    query_response_as_json_value(resp)
}

fn tool_to_query(tool: &str, args: Value) -> Result<QueryRequest, McpToolError> {
    match tool {
        TOOL_GRAPH_CALLERS => Ok(QueryRequest {
            op: QueryOp::Callers,
            id: Some(get_string(&args, "id")?),
            query: None,
            limit: None,
            depth: None,
            scope: None,
            boundary: None,
        }),
        TOOL_GRAPH_TRACE => Ok(QueryRequest {
            op: QueryOp::Trace,
            id: Some(get_string(&args, "id")?),
            query: None,
            limit: None,
            depth: get_optional_usize(&args, "depth"),
            scope: None,
            boundary: None,
        }),
        TOOL_GRAPH_SEARCH => Ok(QueryRequest {
            op: QueryOp::Search,
            id: None,
            query: Some(get_string(&args, "query")?),
            limit: get_optional_usize(&args, "limit"),
            depth: None,
            scope: None,
            boundary: None,
        }),
        TOOL_GRAPH_ENTRY_POINTS => Ok(QueryRequest {
            op: QueryOp::EntryPoints,
            id: None,
            query: None,
            limit: None,
            depth: None,
            scope: None,
            boundary: None,
        }),
        TOOL_GRAPH_IMPACT => Ok(QueryRequest {
            op: QueryOp::Impact,
            id: Some(get_string(&args, "id")?),
            query: None,
            limit: None,
            depth: get_optional_usize(&args, "depth"),
            scope: None,
            boundary: None,
        }),
        other => Err(McpToolError::UnknownTool(other.to_string())),
    }
}

fn get_string(args: &Value, key: &str) -> Result<String, McpToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| McpToolError::BadArgs(format!("missing or empty `{key}`")))
}

fn get_optional_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

/// Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0).
pub async fn run_mcp_stdio(state: AppState, repo_path: &Path) -> io::Result<()> {
    state
        .index_path(repo_path)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("{e:?}")))?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(err) => {
                write_response(
                    &mut stdout,
                    None,
                    json!({
                        "code": -32700,
                        "message": format!("parse error: {err}")
                    }),
                )?;
                continue;
            }
        };

        let id = request.id.clone();
        let result = handle_request(&state, request).await;
        match result {
            Ok(value) => write_response(&mut stdout, id, value)?,
            Err(err) => {
                write_response(
                    &mut stdout,
                    id,
                    json!({
                        "code": -32000,
                        "message": err.to_string()
                    }),
                )?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

async fn handle_request(state: &AppState, request: JsonRpcRequest) -> Result<Value, McpToolError> {
    match request.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        })),
        "notifications/initialized" | "initialized" => Ok(Value::Null),
        "tools/list" => Ok(json!({
            "tools": [
                tool_schema(TOOL_GRAPH_SEARCH, "Search symbols by name or path", json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer" }
                    }
                })),
                tool_schema(TOOL_GRAPH_CALLERS, "Symbols that call the given node id", json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
                    }
                })),
                tool_schema(TOOL_GRAPH_TRACE, "Trace CALLS edges from a root symbol", json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" },
                        "depth": { "type": "integer" }
                    }
                })),
                tool_schema(TOOL_GRAPH_ENTRY_POINTS, "List graph entry points (roots)", json!({
                    "type": "object",
                    "properties": {}
                })),
                tool_schema(TOOL_GRAPH_IMPACT, "Blast radius: who transitively calls this symbol", json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" },
                        "depth": { "type": "integer" }
                    }
                }))
            ]
        })),
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| McpToolError::BadArgs("tools/call requires name".into()))?;
            let args = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));
            let payload = call_mcp_tool_on_state(state, name, args)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                }],
                "structuredContent": payload
            }))
        }
        _ => Err(McpToolError::BadArgs(format!(
            "unsupported method: {}",
            request.method
        ))),
    }
}

fn tool_schema(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

fn write_response(stdout: &mut impl Write, id: Option<Value>, result: Value) -> io::Result<()> {
    let body = if result.get("code").is_some() {
        json!({ "jsonrpc": "2.0", "id": id, "error": result })
    } else if result.is_null() {
        return Ok(());
    } else {
        json!({ "jsonrpc": "2.0", "id": id, "result": result })
    };
    writeln!(stdout, "{}", body)
}

