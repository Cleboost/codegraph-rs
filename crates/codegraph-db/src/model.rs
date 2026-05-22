use camino::Utf8PathBuf;
use codegraph_core::{EdgeKind, NodeKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRow {
    pub id: Option<i64>,
    pub path: Utf8PathBuf,
    pub language: String,
    pub sha256: String,
    pub size: u64,
    pub mtime: i64,
    pub indexed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDraft {
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDraft {
    pub from_id: i64,
    pub to_id: i64,
    pub kind: EdgeKind,
    pub file_id: Option<i64>,
    pub line: Option<u32>,
    pub source: Option<String>, // e.g. "framework:express", "resolver:imports"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStats {
    pub files: u64,
    pub nodes: u64,
    pub edges: u64,
    pub size_bytes: u64,
    pub schema_version: u32,
}
