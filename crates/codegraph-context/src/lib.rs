//! Context builder: search → callers + callees → markdown/json.

use codegraph_core::{Node, Result};
use codegraph_db::Db;
use codegraph_graph::Traversal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    let candidates = db.search_nodes(&req.query, req.limit)?;
    let trav = Traversal::new(db);

    // Pre-load each unique file once when source is requested.
    let file_cache: HashMap<String, Vec<String>> = if req.include_source {
        let mut cache = HashMap::new();
        for n in &candidates {
            let key = n.file.as_str().to_owned();
            if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(key) {
                if let Ok(text) = std::fs::read_to_string(n.file.as_std_path()) {
                    e.insert(text.lines().map(str::to_owned).collect());
                }
            }
        }
        cache
    } else {
        HashMap::new()
    };

    let mut hits = Vec::new();
    for n in candidates {
        let callers = trav.callers(n.id, req.depth)?.nodes;
        let callees = trav.callees(n.id, req.depth)?.nodes;
        let source = if req.include_source {
            file_cache.get(n.file.as_str()).map(|lines| {
                let start = n.start_line.saturating_sub(1) as usize;
                let end = (n.end_line as usize).min(lines.len());
                lines[start..end].join("\n")
            })
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
