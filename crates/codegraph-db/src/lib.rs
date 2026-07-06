//! SQLite-backed knowledge graph storage. rusqlite bundled + FTS5.
//!
//! Schema v1, no compat with archive TS DB.

mod migrations;
mod model;
mod queries;

pub use model::{DbStats, EdgeDraft, FileRow, NodeDraft};

use camino::{Utf8Path, Utf8PathBuf};
use codegraph_core::{Edge, EdgeKind, Error, Node, NodeId, NodeKind, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};

pub const SCHEMA_SQL: &str = include_str!("schema.sql");
pub const SCHEMA_VERSION: u32 = 1;

pub struct Db {
    conn: Mutex<Connection>,
    path: Utf8PathBuf,
}

impl Db {
    pub fn open(path: &Utf8Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path).map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db_err)?;
        migrations::run(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    pub fn open_read_only(path: &Utf8Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Project root when the DB lives at `{root}/.codegraph/db.sqlite`.
    pub fn workspace_root(&self) -> Option<&Utf8Path> {
        let codegraph_dir = self.path.parent()?;
        if codegraph_dir.file_name() != Some(".codegraph") {
            return None;
        }
        codegraph_dir.parent()
    }

    pub fn schema_version(&self) -> Result<u32> {
        let c = self.conn.lock();
        queries::schema_version(&c)
    }

    pub fn upsert_file(&self, f: &FileRow) -> Result<i64> {
        let mut c = self.conn.lock();
        let tx = c.transaction().map_err(db_err)?;
        let id = queries::upsert_file(&tx, f)?;
        tx.commit().map_err(db_err)?;
        Ok(id)
    }

    pub fn delete_file_cascade(&self, file_id: i64) -> Result<()> {
        let c = self.conn.lock();
        c.execute("DELETE FROM files WHERE id = ?", [file_id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn insert_nodes(&self, file_id: i64, drafts: &[NodeDraft]) -> Result<Vec<NodeId>> {
        let mut c = self.conn.lock();
        let tx = c.transaction().map_err(db_err)?;
        let ids = queries::insert_nodes(&tx, file_id, drafts)?;
        tx.commit().map_err(db_err)?;
        Ok(ids)
    }

    pub fn insert_edges(&self, edges: &[EdgeDraft]) -> Result<()> {
        let mut c = self.conn.lock();
        let tx = c.transaction().map_err(db_err)?;
        queries::insert_edges(&tx, edges)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn search_nodes(&self, query: &str, limit: u32) -> Result<Vec<Node>> {
        let c = self.conn.lock();
        queries::search_fts(&c, query, limit)
    }

    pub fn node_by_id(&self, id: NodeId) -> Result<Option<Node>> {
        let c = self.conn.lock();
        queries::node_by_id(&c, id)
    }

    pub fn nodes_by_name(&self, name: &str) -> Result<Vec<Node>> {
        let c = self.conn.lock();
        queries::nodes_by_name(&c, name)
    }

    pub fn callers_of(&self, id: NodeId) -> Result<Vec<Edge>> {
        let c = self.conn.lock();
        queries::edges_to(&c, id, EdgeKind::Calls)
    }

    pub fn callees_of(&self, id: NodeId) -> Result<Vec<Edge>> {
        let c = self.conn.lock();
        queries::edges_from(&c, id, EdgeKind::Calls)
    }

    pub fn edges_from(&self, id: NodeId, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        let c = self.conn.lock();
        queries::edges_from_any(&c, id, kinds)
    }

    pub fn edges_to(&self, id: NodeId, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        let c = self.conn.lock();
        queries::edges_to_any(&c, id, kinds)
    }

    pub fn files_under(&self, prefix: &str) -> Result<Vec<FileRow>> {
        let prefix = normalize_files_prefix(self, prefix);
        let c = self.conn.lock();
        queries::files_under(&c, &prefix)
    }

    pub fn file_by_path(&self, path: &str) -> Result<Option<FileRow>> {
        let c = self.conn.lock();
        queries::file_by_path(&c, path)
    }

    pub fn update_file_metadata(&self, path: &str, mtime: i64, size: u64) -> Result<()> {
        let c = self.conn.lock();
        queries::update_file_metadata(&c, path, mtime, size)
    }

    pub fn file_by_id(&self, id: i64) -> Result<Option<FileRow>> {
        let c = self.conn.lock();
        queries::file_by_id(&c, id)
    }

    pub fn stats(&self) -> Result<DbStats> {
        let c = self.conn.lock();
        queries::stats(&c)
    }

    pub fn nodes_by_file_ids(&self, file_ids: &[i64], limit: u32) -> Result<Vec<Node>> {
        let c = self.conn.lock();
        queries::nodes_by_file_ids(&c, file_ids, limit)
    }

    pub fn nodes_under_prefix(&self, prefix: &str, limit: u32) -> Result<Vec<Node>> {
        let prefix = normalize_files_prefix(self, prefix);
        let c = self.conn.lock();
        queries::nodes_under_prefix(&c, &prefix, limit)
    }

    pub fn edges_between(
        &self,
        node_ids: &[NodeId],
        kinds: &[EdgeKind],
        limit: u32,
    ) -> Result<Vec<Edge>> {
        let c = self.conn.lock();
        queries::edges_between(&c, node_ids, kinds, limit)
    }

    pub fn purge(&self) -> Result<()> {
        let c = self.conn.lock();
        c.execute_batch("DELETE FROM edges; DELETE FROM nodes; DELETE FROM files;")
            .map_err(db_err)?;
        Ok(())
    }
}

pub(crate) fn db_err(e: rusqlite::Error) -> Error {
    Error::Db(e.to_string())
}

fn normalize_files_prefix(db: &Db, prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }

    let path = Utf8Path::new(prefix);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(root) = db.workspace_root() {
        root.join(path)
    } else {
        return forward_slashes(prefix);
    };

    let normalized = lexical_normalize(&resolved);
    let canonical = normalized.canonicalize_utf8().unwrap_or(normalized);
    forward_slashes(&canonical)
}

fn forward_slashes(path: impl AsRef<str>) -> String {
    path.as_ref().replace('\\', "/")
}

fn lexical_normalize(path: &Utf8Path) -> Utf8PathBuf {
    use camino::Utf8Component;

    let mut out = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::Prefix(prefix) => {
                out = Utf8PathBuf::from(prefix.as_str());
            }
            Utf8Component::RootDir => {
                out.push("/");
            }
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                out.pop();
            }
            Utf8Component::Normal(segment) => {
                out.push(segment);
            }
        }
    }
    out
}

pub(crate) fn kind_str(k: NodeKind) -> &'static str {
    k.as_str()
}
pub(crate) fn ekind_str(k: EdgeKind) -> &'static str {
    k.as_str()
}
