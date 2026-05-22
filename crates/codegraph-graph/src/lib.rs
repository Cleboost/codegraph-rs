//! Graph traversal: callers, callees, impact radius.

use codegraph_core::{Edge, EdgeKind, Node, NodeId, Result};
use codegraph_db::Db;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

const HARD_LIMIT: usize = 5000;

pub struct Traversal<'a> { db: &'a Db }

impl<'a> Traversal<'a> {
    pub fn new(db: &'a Db) -> Self { Self { db } }

    pub fn callers(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        self.traverse(id, depth, &[EdgeKind::Calls], false)
    }

    pub fn callees(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        self.traverse(id, depth, &[EdgeKind::Calls], true)
    }

    /// Forward impact across calls/references/imports/extends/implements.
    pub fn impact_radius(&self, id: NodeId, max_depth: u32) -> Result<ImpactReport> {
        let kinds = [
            EdgeKind::Calls, EdgeKind::References, EdgeKind::Imports,
            EdgeKind::Extends, EdgeKind::Implements,
        ];
        let hits = self.traverse(id, max_depth, &kinds, false)?; // who depends on us = incoming
        let root = self.db.node_by_id(id)?.ok_or_else(|| {
            codegraph_core::Error::Invalid(format!("node {id} not found"))
        })?;
        let mut by_kind: HashMap<String, u32> = HashMap::new();
        for n in &hits.nodes {
            *by_kind.entry(n.kind.as_str().into()).or_insert(0) += 1;
        }
        let mut direct = Vec::new();
        let mut transitive = Vec::new();
        for (n, d) in hits.nodes.iter().zip(hits.depths.iter()) {
            if *d == 1 { direct.push(n.clone()); } else { transitive.push(n.clone()); }
        }
        Ok(ImpactReport {
            root, direct, transitive, by_kind, truncated: hits.truncated,
        })
    }

    fn traverse(
        &self,
        start: NodeId,
        max_depth: u32,
        kinds: &[EdgeKind],
        forward: bool,
    ) -> Result<TraverseHits> {
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();
        let mut nodes = Vec::new();
        let mut depths = Vec::new();
        let mut edges = Vec::new();
        let mut truncated = false;
        visited.insert(start);
        queue.push_back((start, 0));

        while let Some((cur, d)) = queue.pop_front() {
            if d >= max_depth { continue; }
            if visited.len() > HARD_LIMIT { truncated = true; break; }
            let next_edges = if forward {
                self.db.edges_from(cur, kinds)?
            } else {
                self.db.edges_to(cur, kinds)?
            };
            for e in next_edges {
                let next_id = if forward { e.to } else { e.from };
                edges.push(e);
                if visited.insert(next_id) {
                    if let Some(n) = self.db.node_by_id(next_id)? {
                        nodes.push(n);
                        depths.push(d + 1);
                    }
                    queue.push_back((next_id, d + 1));
                }
            }
        }

        Ok(TraverseHits { nodes, depths, edges, truncated })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraverseHits {
    pub nodes: Vec<Node>,
    pub depths: Vec<u32>,
    pub edges: Vec<Edge>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub root: Node,
    pub direct: Vec<Node>,
    pub transitive: Vec<Node>,
    pub by_kind: HashMap<String, u32>,
    pub truncated: bool,
}
