//! Reference resolution: name-match pending calls into actual `calls` edges.
//!
//! Strategy: for each PendingCall { from, target_name, line }, look up nodes
//! named `target_name` of kind function|method, then pick the closest match
//! by proximity score (same file > same directory > anywhere).

pub mod frameworks;
pub mod imports;
pub mod name_match;

use codegraph_core::{Node, EdgeKind, NodeId, Result};
use codegraph_db::{Db, EdgeDraft};
use std::collections::HashMap;
use std::path::PathBuf;

/// Input from an extractor pass: ready-to-resolve call sites.
#[derive(Debug, Clone)]
pub struct PendingCallRow {
    pub from_id: NodeId,
    pub target_name: String,
    pub file_id: i64,
    pub line: u32,
}

pub struct Resolver<'a> {
    db: &'a Db,
}

impl<'a> Resolver<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn resolve_calls(&self, pending: &[PendingCallRow]) -> Result<usize> {
        if pending.is_empty() {
            return Ok(0);
        }

        let mut file_cache: HashMap<i64, Option<(String, PathBuf)>> = HashMap::new();
        for p in pending {
            file_cache.entry(p.file_id).or_insert_with(|| {
                self.db.file_by_id(p.file_id).ok().flatten().and_then(|f| {
                    let dir = std::path::Path::new(f.path.as_str())
                        .parent()
                        .map(|d| d.to_path_buf())?;
                    Some((f.path.to_string(), dir))
                })
            });
        }

        let mut by_name: HashMap<&str, Vec<&PendingCallRow>> = HashMap::new();
        for p in pending {
            by_name.entry(p.target_name.as_str()).or_default().push(p);
        }

        let mut edges: Vec<EdgeDraft> = Vec::new();
        for (name, sites) in by_name {
            let candidates = self.db.nodes_by_name(name)?;
            if candidates.is_empty() {
                continue;
            }
            let callable: Vec<_> = candidates
                .into_iter()
                .filter(|n| {
                    matches!(
                        n.kind,
                        codegraph_core::NodeKind::Function | codegraph_core::NodeKind::Method
                    )
                })
                .collect();
            if callable.is_empty() {
                continue;
            }

            for site in sites {
                let caller_info = file_cache.get(&site.file_id).and_then(|v| v.as_ref());
                let best_score = callable
                    .iter()
                    .map(|n| proximity_score(n, caller_info))
                    .max()
                    .unwrap_or(1);
                let targets: Vec<_> = callable
                    .iter()
                    .filter(|n| proximity_score(n, caller_info) == best_score)
                    .collect();
                for t in targets {
                    edges.push(EdgeDraft {
                        from_id: site.from_id,
                        to_id: t.id,
                        kind: EdgeKind::Calls,
                        file_id: Some(site.file_id),
                        line: Some(site.line),
                        source: Some("resolver:name-match".into()),
                    });
                }
            }
        }
        let n = edges.len();
        self.db.insert_edges(&edges)?;
        Ok(n)
    }
}

/// Score a candidate node by proximity to the caller.
/// 3 = same file, 2 = same directory, 1 = elsewhere.
fn proximity_score(candidate: &Node, caller_info: Option<&(String, PathBuf)>) -> u8 {
    let Some((caller_path, caller_dir)) = caller_info else {
        return 1;
    };
    if candidate.file.as_str() == caller_path.as_str() {
        return 3;
    }
    let candidate_dir = std::path::Path::new(candidate.file.as_str()).parent();
    if candidate_dir == Some(caller_dir.as_path()) {
        2
    } else {
        1
    }
}
