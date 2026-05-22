//! MCP server (stdio JSON-RPC 2.0). Hand-rolled — no MCP SDK dep.
//!
//! Tools exposed: codegraph_search, codegraph_node, codegraph_callers,
//! codegraph_callees, codegraph_impact, codegraph_context, codegraph_explore,
//! codegraph_files, codegraph_status.
//!
//! TODO: transport (stdio framing), dispatch, tool handlers, server-instructions string.

pub const SERVER_INSTRUCTIONS: &str = include_str!("server-instructions.md");
