//! Graph traversal: callers, callees, impact radius.

use codegraph_core::{Edge, EdgeKind, Node, NodeId, Result};
use codegraph_db::Db;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub const DEFAULT_NODE_LIMIT: u32 = 2000;
pub const DEFAULT_EDGE_LIMIT: u32 = 5000;
const HARD_LIMIT: usize = 5000;

pub const VIZ_EDGE_KINDS: [EdgeKind; 9] = [
    EdgeKind::Calls,
    EdgeKind::Imports,
    EdgeKind::Extends,
    EdgeKind::Implements,
    EdgeKind::References,
    EdgeKind::TypeOf,
    EdgeKind::Instantiates,
    EdgeKind::Overrides,
    EdgeKind::Decorates,
];

pub struct Traversal<'a> {
    db: &'a Db,
}

impl<'a> Traversal<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn callers(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        self.traverse(id, depth, &[EdgeKind::Calls], false)
    }

    pub fn callees(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        self.traverse(id, depth, &[EdgeKind::Calls], true)
    }

    /// BFS in both directions around a node.
    pub fn neighborhood(&self, id: NodeId, depth: u32, kinds: &[EdgeKind]) -> Result<TraverseHits> {
        let root = self
            .db
            .node_by_id(id)?
            .ok_or_else(|| codegraph_core::Error::Invalid(format!("node {id} not found")))?;
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();
        let mut nodes = Vec::new();
        let mut depths = Vec::new();
        let mut edges = Vec::new();
        let mut truncated = false;
        visited.insert(id);
        queue.push_back((id, 0));

        while let Some((cur, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }
            if visited.len() > HARD_LIMIT {
                truncated = true;
                break;
            }
            let out_edges = self.db.edges_from(cur, kinds)?;
            let in_edges = self.db.edges_to(cur, kinds)?;
            for e in out_edges.into_iter().chain(in_edges) {
                let next_id = if e.from == cur { e.to } else { e.from };
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

        Ok(TraverseHits {
            root: Some(root),
            nodes,
            depths,
            edges,
            truncated,
        })
    }

    pub fn subgraph(&self, req: SubgraphRequest) -> Result<SubgraphResponse> {
        let kinds = if req.kinds.is_empty() {
            VIZ_EDGE_KINDS.to_vec()
        } else {
            req.kinds.clone()
        };
        let node_limit = req.node_limit.unwrap_or(DEFAULT_NODE_LIMIT);
        let edge_limit = req.edge_limit.unwrap_or(DEFAULT_EDGE_LIMIT);

        if let Some(seed) = req.seed {
            let mut hits = self.neighborhood(seed, req.depth, &kinds)?;
            if hits.nodes.len() as u32 > node_limit {
                hits.nodes.truncate(node_limit as usize);
                hits.depths.truncate(node_limit as usize);
                hits.truncated = true;
            }
            if hits.edges.len() as u32 > edge_limit {
                hits.edges.truncate(edge_limit as usize);
                hits.truncated = true;
            }
            return Ok(SubgraphResponse {
                seed: hits.root.clone(),
                nodes: hits.nodes,
                edges: hits.edges,
                truncated: hits.truncated,
            });
        }

        if let Some(prefix) = req.prefix {
            let files = self.db.files_under(&prefix)?;
            let file_ids: Vec<i64> = files.iter().filter_map(|f| f.id).collect();
            let nodes = self.db.nodes_by_file_ids(&file_ids, node_limit)?;
            let mut truncated = nodes.len() as u32 >= node_limit;
            let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
            let edges = self.db.edges_between(&node_ids, &kinds, edge_limit)?;
            if edges.len() as u32 >= edge_limit {
                truncated = true;
            }
            return Ok(SubgraphResponse {
                seed: None,
                nodes,
                edges,
                truncated,
            });
        }

        if let Some(query) = req.query {
            let hits = self.db.search_nodes(&query, 1)?;
            let seed = hits.into_iter().next().ok_or_else(|| {
                codegraph_core::Error::Invalid(format!("no node matching '{query}'"))
            })?;
            let mut sub = self.neighborhood(seed.id, req.depth, &kinds)?;
            if sub.nodes.len() as u32 > node_limit {
                sub.nodes.truncate(node_limit as usize);
                sub.truncated = true;
            }
            if sub.edges.len() as u32 > edge_limit {
                sub.edges.truncate(edge_limit as usize);
                sub.truncated = true;
            }
            return Ok(SubgraphResponse {
                seed: sub.root.clone(),
                nodes: sub.nodes,
                edges: sub.edges,
                truncated: sub.truncated,
            });
        }

        // Default: capped overview of the whole workspace (prefix "").
        let files = self.db.files_under("")?;
        let file_ids: Vec<i64> = files.iter().filter_map(|f| f.id).collect();
        let nodes = self.db.nodes_by_file_ids(&file_ids, node_limit)?;
        let mut truncated = nodes.len() as u32 >= node_limit;
        let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
        let edges = self.db.edges_between(&node_ids, &kinds, edge_limit)?;
        if edges.len() as u32 >= edge_limit {
            truncated = true;
        }
        Ok(SubgraphResponse {
            seed: None,
            nodes,
            edges,
            truncated,
        })
    }

    /// All nodes that reference this node (depth 1, all non-containment edge kinds).
    pub fn references(&self, id: NodeId) -> Result<ReferencesReport> {
        let kinds = [
            EdgeKind::Calls,
            EdgeKind::Imports,
            EdgeKind::Extends,
            EdgeKind::Implements,
            EdgeKind::References,
            EdgeKind::TypeOf,
            EdgeKind::Instantiates,
            EdgeKind::Overrides,
            EdgeKind::Decorates,
        ];
        let root = self
            .db
            .node_by_id(id)?
            .ok_or_else(|| codegraph_core::Error::Invalid(format!("node {id} not found")))?;
        let edges = self.db.edges_to(id, &kinds)?;
        let mut by_kind: HashMap<String, Vec<Node>> = HashMap::new();
        for e in &edges {
            if let Some(n) = self.db.node_by_id(e.from)? {
                by_kind.entry(e.kind.as_str().into()).or_default().push(n);
            }
        }
        Ok(ReferencesReport { root, by_kind })
    }

    /// Forward impact across calls/references/imports/extends/implements.
    pub fn impact_radius(&self, id: NodeId, max_depth: u32) -> Result<ImpactReport> {
        let kinds = [
            EdgeKind::Calls,
            EdgeKind::References,
            EdgeKind::Imports,
            EdgeKind::Extends,
            EdgeKind::Implements,
        ];
        let hits = self.traverse(id, max_depth, &kinds, false)?; // who depends on us = incoming
        let root = self
            .db
            .node_by_id(id)?
            .ok_or_else(|| codegraph_core::Error::Invalid(format!("node {id} not found")))?;
        let mut by_kind: HashMap<String, u32> = HashMap::new();
        for n in &hits.nodes {
            *by_kind.entry(n.kind.as_str().into()).or_insert(0) += 1;
        }
        let mut direct = Vec::new();
        let mut transitive = Vec::new();
        for (n, d) in hits.nodes.iter().zip(hits.depths.iter()) {
            if *d == 1 {
                direct.push(n.clone());
            } else {
                transitive.push(n.clone());
            }
        }
        Ok(ImpactReport {
            root,
            direct,
            transitive,
            by_kind,
            truncated: hits.truncated,
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
            if d >= max_depth {
                continue;
            }
            if visited.len() > HARD_LIMIT {
                truncated = true;
                break;
            }
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

        Ok(TraverseHits {
            root: None,
            nodes,
            depths,
            edges,
            truncated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraverseHits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Node>,
    pub nodes: Vec<Node>,
    pub depths: Vec<u32>,
    pub edges: Vec<Edge>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphRequest {
    pub seed: Option<NodeId>,
    pub query: Option<String>,
    pub prefix: Option<String>,
    pub depth: u32,
    #[serde(default)]
    pub kinds: Vec<EdgeKind>,
    pub node_limit: Option<u32>,
    pub edge_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<Node>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencesReport {
    pub root: Node,
    /// Inbound references grouped by edge kind (calls, imports, extends, …).
    pub by_kind: HashMap<String, Vec<Node>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub root: Node,
    pub direct: Vec<Node>,
    pub transitive: Vec<Node>,
    pub by_kind: HashMap<String, u32>,
    pub truncated: bool,
}
