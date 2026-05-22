use crate::{EdgeKind, NodeKind};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

pub type NodeId = i64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: Option<String>,
    pub file: Utf8PathBuf,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub file: Option<Utf8PathBuf>,
    pub line: Option<u32>,
}
