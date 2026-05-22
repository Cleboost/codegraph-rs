//! Context builder: search → callers + callees → markdown/json.

use codegraph_core::{Node, Result};
use codegraph_db::Db;
use codegraph_graph::Traversal;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Markdown,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub query: String,
    pub depth: u32,
    pub include_source: bool,
    pub limit: u32,
    pub format: Format,
}

impl Default for ContextRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            depth: 1,
            include_source: false,
            limit: 5,
            format: Format::Markdown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextHit {
    pub node: Node,
    pub callers: Vec<Node>,
    pub callees: Vec<Node>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResponse {
    pub query: String,
    pub hits: Vec<ContextHit>,
}

pub fn build(db: &Db, req: &ContextRequest) -> Result<String> {
    let response = build_response(db, req)?;
    match req.format {
        Format::Json => Ok(serde_json::to_string_pretty(&response).unwrap_or_default()),
        Format::Markdown => Ok(render_markdown(&response)),
    }
}

pub fn build_response(db: &Db, req: &ContextRequest) -> Result<ContextResponse> {
    let mut hits = Vec::new();
    let candidates = db.search_nodes(&req.query, req.limit)?;
    let trav = Traversal::new(db);
    for n in candidates {
        let callers = trav.callers(n.id, req.depth)?.nodes;
        let callees = trav.callees(n.id, req.depth)?.nodes;
        let source = if req.include_source {
            read_source_slice(&n)
        } else {
            None
        };
        hits.push(ContextHit {
            node: n,
            callers,
            callees,
            source,
        });
    }
    Ok(ContextResponse {
        query: req.query.clone(),
        hits,
    })
}

fn read_source_slice(n: &Node) -> Option<String> {
    let text = std::fs::read_to_string(n.file.as_std_path()).ok()?;
    let start = n.start_line.saturating_sub(1) as usize;
    let end = (n.end_line as usize).min(text.lines().count());
    Some(
        text.lines()
            .skip(start)
            .take(end - start)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn render_markdown(resp: &ContextResponse) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Context: `{}`", resp.query);
    if resp.hits.is_empty() {
        let _ = writeln!(out, "\n_No matches._");
        return out;
    }
    for h in &resp.hits {
        let _ = writeln!(
            out,
            "\n## `{}` — {} — `{}:{}`",
            h.node.name,
            h.node.kind.as_str(),
            h.node.file,
            h.node.start_line
        );
        if let Some(sig) = &h.node.signature {
            let _ = writeln!(out, "\n```{}\n{}\n```", h.node.language, sig);
        }
        if let Some(src) = &h.source {
            let _ = writeln!(out, "\n```{}\n{}\n```", h.node.language, src);
        }
        if !h.callers.is_empty() {
            let _ = writeln!(out, "\n**Callers** ({}):", h.callers.len());
            for c in &h.callers {
                let _ = writeln!(out, "- `{}` — `{}:{}`", c.name, c.file, c.start_line);
            }
        }
        if !h.callees.is_empty() {
            let _ = writeln!(out, "\n**Callees** ({}):", h.callees.len());
            for c in &h.callees {
                let _ = writeln!(out, "- `{}` — `{}:{}`", c.name, c.file, c.start_line);
            }
        }
    }
    out
}
