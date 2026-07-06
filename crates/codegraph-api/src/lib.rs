//! Shared graph query API for MCP and visualize HTTP server.

use codegraph_context::{build, ContextRequest};
use codegraph_core::{Node, NodeId, Result};
use codegraph_db::{Db, FileRow};
use codegraph_graph::{
    ImpactReport, ReferencesReport, SubgraphRequest, SubgraphResponse, Traversal, TraverseHits,
};

pub struct GraphApi<'a> {
    db: &'a Db,
}

impl<'a> GraphApi<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Node>> {
        self.db.search_nodes(query, limit)
    }

    pub fn node_by_id(&self, id: NodeId) -> Result<Option<Node>> {
        self.db.node_by_id(id)
    }

    pub fn nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        self.db.nodes_by_name(name)
    }

    pub fn callers(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        Traversal::new(self.db).callers(id, depth)
    }

    pub fn callees(&self, id: NodeId, depth: u32) -> Result<TraverseHits> {
        Traversal::new(self.db).callees(id, depth)
    }

    pub fn impact(&self, id: NodeId, max_depth: u32) -> Result<ImpactReport> {
        Traversal::new(self.db).impact_radius(id, max_depth)
    }

    pub fn references(&self, id: NodeId) -> Result<ReferencesReport> {
        Traversal::new(self.db).references(id)
    }

    pub fn context_markdown(&self, req: &ContextRequest) -> Result<String> {
        build(self.db, req)
    }

    pub fn files(&self, prefix: &str) -> Result<Vec<FileRow>> {
        self.db.files_under(prefix)
    }

    pub fn stats(&self) -> Result<codegraph_db::DbStats> {
        self.db.stats()
    }

    pub fn subgraph(&self, req: SubgraphRequest) -> Result<SubgraphResponse> {
        Traversal::new(self.db).subgraph(req)
    }

    pub fn neighborhood(
        &self,
        id: NodeId,
        depth: u32,
        kinds: &[codegraph_core::EdgeKind],
    ) -> Result<TraverseHits> {
        Traversal::new(self.db).neighborhood(id, depth, kinds)
    }
}
