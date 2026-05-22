//! Reference resolution: name-match pending calls into actual `calls` edges.
//!
//! MVP: in-process resolver invoked by the orchestrator after a batch.
//! Strategy: for each PendingCall { from, target_name, line }, look up nodes
//! named `target_name` of kind function|method. If exactly one match in the
//! same file, link directly. If multiple, link to all (cheap recall over
//! precision).

pub mod frameworks;
pub mod imports;
pub mod name_match;

use codegraph_core::{EdgeKind, NodeId, Result};
use codegraph_db::{Db, EdgeDraft};
use std::collections::HashMap;

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
        // Group by target_name to batch lookups.
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
            // Filter to callable kinds.
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
                // Prefer same-file match. If none, link all callable (recall over precision).
                let same_file: Vec<_> = callable
                    .iter()
                    .filter(|n| {
                        self.db
                            .file_by_path(n.file.as_str())
                            .ok()
                            .flatten()
                            .and_then(|f| f.id)
                            .map(|id| id == site.file_id)
                            .unwrap_or(false)
                    })
                    .collect();
                let targets: Vec<_> = if !same_file.is_empty() {
                    same_file.into_iter().collect()
                } else {
                    callable.iter().collect()
                };
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
