//! MCP server (stdio JSON-RPC 2.0). Hand-rolled, no SDK.

mod protocol;
mod tools;

pub use protocol::{Response, ErrorObj, JsonRpcMessage};
pub use tools::tool_definitions;

use codegraph_db::Db;
use codegraph_graph::Traversal;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub const SERVER_INSTRUCTIONS: &str = include_str!("server-instructions.md");
pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "codegraph";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct McpServer {
    db: Arc<Db>,
}

impl McpServer {
    pub fn new(db: Arc<Db>) -> Self { Self { db } }

    pub async fn run_stdio(self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 { break; }
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }

            let msg: JsonRpcMessage = match serde_json::from_str(trimmed) {
                Ok(m) => m,
                Err(e) => {
                    write_response(&mut stdout, Response::error(Value::Null, -32700, &format!("parse error: {e}"))).await?;
                    continue;
                }
            };
            if msg.id.is_none() {
                // notification — no response
                continue;
            }
            let id = msg.id.clone().unwrap_or(Value::Null);
            let resp = self.dispatch(msg).await;
            let final_resp = match resp {
                Ok(v) => Response::ok(id, v),
                Err(e) => Response::error(id, -32603, &e.to_string()),
            };
            write_response(&mut stdout, final_resp).await?;
        }
        Ok(())
    }

    async fn dispatch(&self, msg: JsonRpcMessage) -> anyhow::Result<Value> {
        match msg.method.as_deref() {
            Some("initialize") => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
                "instructions": SERVER_INSTRUCTIONS,
            })),
            Some("ping") => Ok(json!({})),
            Some("tools/list") => Ok(json!({ "tools": tool_definitions() })),
            Some("tools/call") => self.handle_tool_call(msg.params.unwrap_or(Value::Null)).await,
            Some(m) => Err(anyhow::anyhow!("method not found: {m}")),
            None => Err(anyhow::anyhow!("missing method")),
        }
    }

    async fn handle_tool_call(&self, params: Value) -> anyhow::Result<Value> {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(Value::Null);
        let text = tools::dispatch(&self.db, name, args)?;
        Ok(json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false,
        }))
    }
}

async fn write_response<W: tokio::io::AsyncWrite + Unpin>(w: &mut W, r: Response) -> anyhow::Result<()> {
    let s = serde_json::to_string(&r)?;
    w.write_all(s.as_bytes()).await?;
    w.write_all(b"\n").await?;
    w.flush().await?;
    Ok(())
}

// Re-export for binary use without exposing Traversal lifetime annoyances.
pub fn traversal_for(db: &Db) -> Traversal<'_> { Traversal::new(db) }
