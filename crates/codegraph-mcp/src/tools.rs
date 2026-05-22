use codegraph_context::{build, ContextRequest, Format};
use codegraph_db::Db;
use codegraph_graph::{ReferencesReport, Traversal};
use serde_json::{json, Value};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "codegraph_search",
            "Search the knowledge graph by name / signature / docstring (FTS5).",
            json!({ "type": "object", "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "default": 20 }
            }, "required": ["query"] }),
        ),
        tool(
            "codegraph_node",
            "Look up a node by id or exact name.",
            json!({ "type": "object", "properties": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            } }),
        ),
        tool(
            "codegraph_callers",
            "Find functions that call the given node.",
            json!({ "type": "object", "properties": {
                "node": { "type": "integer" },
                "depth": { "type": "integer", "default": 1 }
            }, "required": ["node"] }),
        ),
        tool(
            "codegraph_callees",
            "Find functions called by the given node.",
            json!({ "type": "object", "properties": {
                "node": { "type": "integer" },
                "depth": { "type": "integer", "default": 1 }
            }, "required": ["node"] }),
        ),
        tool(
            "codegraph_impact",
            "Impact radius: who transitively depends on this node.",
            json!({ "type": "object", "properties": {
                "node": { "type": "integer" },
                "max_depth": { "type": "integer", "default": 3 }
            }, "required": ["node"] }),
        ),
        tool(
            "codegraph_context",
            "Composed context for a symbol or topic (search + callers + callees).",
            json!({ "type": "object", "properties": {
                "query": { "type": "string" },
                "depth": { "type": "integer", "default": 1 },
                "include_source": { "type": "boolean", "default": false },
                "limit": { "type": "integer", "default": 5 }
            }, "required": ["query"] }),
        ),
        tool(
            "codegraph_references",
            "All nodes that reference this node (calls, imports, extends, implements, type_of, instantiates, …), grouped by relationship kind.",
            json!({ "type": "object", "properties": {
                "node": { "type": "integer", "description": "Node id to find references for" }
            }, "required": ["node"] }),
        ),
        tool(
            "codegraph_files",
            "List indexed files under a path prefix.",
            json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
        ),
        tool(
            "codegraph_status",
            "Index health: counts, size, schema version.",
            json!({ "type": "object", "properties": {} }),
        ),
    ]
}

fn tool(name: &str, desc: &str, schema: Value) -> Value {
    json!({ "name": name, "description": desc, "inputSchema": schema })
}

pub fn dispatch(db: &Db, name: &str, args: Value) -> anyhow::Result<String> {
    match name {
        "codegraph_search" => {
            let q = arg_str(&args, "query")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
            let hits = db.search_nodes(q, limit)?;
            Ok(serde_json::to_string_pretty(&hits)?)
        }
        "codegraph_node" => {
            if let Some(id) = args.get("id").and_then(|v| v.as_i64()) {
                let n = db.node_by_id(id)?;
                return Ok(serde_json::to_string_pretty(&n)?);
            }
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                let n = db.nodes_by_name(name)?;
                return Ok(serde_json::to_string_pretty(&n)?);
            }
            Err(anyhow::anyhow!("provide id or name"))
        }
        "codegraph_callers" => {
            let id = arg_i64(&args, "node")?;
            let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let t = Traversal::new(db);
            Ok(serde_json::to_string_pretty(&t.callers(id, depth)?)?)
        }
        "codegraph_callees" => {
            let id = arg_i64(&args, "node")?;
            let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let t = Traversal::new(db);
            Ok(serde_json::to_string_pretty(&t.callees(id, depth)?)?)
        }
        "codegraph_impact" => {
            let id = arg_i64(&args, "node")?;
            let depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as u32;
            let t = Traversal::new(db);
            Ok(serde_json::to_string_pretty(&t.impact_radius(id, depth)?)?)
        }
        "codegraph_context" => {
            let req = ContextRequest {
                query: arg_str(&args, "query")?.to_string(),
                depth: args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                include_source: args
                    .get("include_source")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                limit: args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32,
                format: Format::Markdown,
            };
            Ok(build(db, &req)?)
        }
        "codegraph_references" => {
            let id = arg_i64(&args, "node")?;
            let t = Traversal::new(db);
            let report: ReferencesReport = t.references(id)?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "codegraph_files" => {
            let prefix = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            Ok(serde_json::to_string_pretty(&db.files_under(prefix)?)?)
        }
        "codegraph_status" => Ok(serde_json::to_string_pretty(&db.stats()?)?),
        _ => Err(anyhow::anyhow!("unknown tool: {name}")),
    }
}

fn arg_str<'a>(v: &'a Value, k: &str) -> anyhow::Result<&'a str> {
    v.get(k)
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing string arg: {k}"))
}
fn arg_i64(v: &Value, k: &str) -> anyhow::Result<i64> {
    v.get(k)
        .and_then(|x| x.as_i64())
        .ok_or_else(|| anyhow::anyhow!("missing int arg: {k}"))
}
