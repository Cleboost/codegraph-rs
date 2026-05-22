use crate::{db_err, ekind_str, kind_str, DbStats, EdgeDraft, FileRow, NodeDraft};
use camino::Utf8PathBuf;
use codegraph_core::{Edge, EdgeKind, Error, Node, NodeId, NodeKind, Result};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

pub(crate) fn schema_version(c: &Connection) -> Result<u32> {
    let v: Option<String> = c
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_err)?;
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

pub(crate) fn upsert_file(tx: &Transaction, f: &FileRow) -> Result<i64> {
    let id: i64 = tx
        .query_row(
            "INSERT INTO files(path, language, sha256, size, mtime, indexed_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET
                language=excluded.language,
                sha256=excluded.sha256,
                size=excluded.size,
                mtime=excluded.mtime,
                indexed_at=excluded.indexed_at
             RETURNING id",
            params![
                f.path.as_str(),
                f.language,
                f.sha256,
                f.size as i64,
                f.mtime,
                f.indexed_at
            ],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    Ok(id)
}

pub(crate) fn file_by_path(c: &Connection, path: &str) -> Result<Option<FileRow>> {
    c.query_row(
        "SELECT id, path, language, sha256, size, mtime, indexed_at FROM files WHERE path=?1",
        [path],
        row_to_file,
    )
    .optional()
    .map_err(db_err)
}

pub(crate) fn file_by_id(c: &Connection, id: i64) -> Result<Option<FileRow>> {
    c.query_row(
        "SELECT id, path, language, sha256, size, mtime, indexed_at FROM files WHERE id=?1",
        [id],
        row_to_file,
    )
    .optional()
    .map_err(db_err)
}

pub(crate) fn files_under(c: &Connection, prefix: &str) -> Result<Vec<FileRow>> {
    let mut s = c
        .prepare_cached(
            "SELECT id, path, language, sha256, size, mtime, indexed_at
             FROM files WHERE path LIKE ?1 ORDER BY path",
        )
        .map_err(db_err)?;
    let pat = format!("{}%", prefix);
    let it = s.query_map([pat], row_to_file).map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn insert_nodes(
    tx: &Transaction,
    file_id: i64,
    drafts: &[NodeDraft],
) -> Result<Vec<NodeId>> {
    let mut s = tx
        .prepare_cached(
            "INSERT INTO nodes(kind, name, qualified_name, file_id, start_line, end_line, signature, docstring, language)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(db_err)?;
    let mut ids = Vec::with_capacity(drafts.len());
    for d in drafts {
        s.execute(params![
            kind_str(d.kind),
            d.name,
            d.qualified_name,
            file_id,
            d.start_line,
            d.end_line,
            d.signature,
            d.docstring,
            d.language,
        ])
        .map_err(db_err)?;
        ids.push(tx.last_insert_rowid());
    }
    Ok(ids)
}

pub(crate) fn insert_edges(tx: &Transaction, edges: &[EdgeDraft]) -> Result<()> {
    let mut s = tx
        .prepare_cached(
            "INSERT INTO edges(from_id, to_id, kind, file_id, line, source)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .map_err(db_err)?;
    for e in edges {
        s.execute(params![
            e.from_id,
            e.to_id,
            ekind_str(e.kind),
            e.file_id,
            e.line,
            e.source,
        ])
        .map_err(db_err)?;
    }
    Ok(())
}

pub(crate) fn node_by_id(c: &Connection, id: NodeId) -> Result<Option<Node>> {
    c.query_row(
        "SELECT n.id, n.kind, n.name, n.qualified_name, f.path, n.start_line, n.end_line,
                n.signature, n.docstring, n.language
         FROM nodes n JOIN files f ON f.id = n.file_id
         WHERE n.id = ?1",
        [id],
        row_to_node,
    )
    .optional()
    .map_err(db_err)
}

pub(crate) fn nodes_by_name(c: &Connection, name: &str) -> Result<Vec<Node>> {
    let mut s = c
        .prepare_cached(
            "SELECT n.id, n.kind, n.name, n.qualified_name, f.path, n.start_line, n.end_line,
                    n.signature, n.docstring, n.language
             FROM nodes n JOIN files f ON f.id = n.file_id
             WHERE n.name = ?1
             ORDER BY n.id LIMIT 100",
        )
        .map_err(db_err)?;
    let it = s.query_map([name], row_to_node).map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn search_fts(c: &Connection, q: &str, limit: u32) -> Result<Vec<Node>> {
    // Escape FTS5 special chars by wrapping each token in double quotes.
    let escaped = q
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ");
    let sql = "SELECT n.id, n.kind, n.name, n.qualified_name, f.path, n.start_line, n.end_line,
                      n.signature, n.docstring, n.language
               FROM nodes_fts ft
               JOIN nodes n ON n.id = ft.rowid
               JOIN files f ON f.id = n.file_id
               WHERE nodes_fts MATCH ?1
               ORDER BY rank
               LIMIT ?2";
    let mut s = c.prepare_cached(sql).map_err(db_err)?;
    let it = s
        .query_map(params![escaped, limit as i64], row_to_node)
        .map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn edges_from(c: &Connection, id: NodeId, kind: EdgeKind) -> Result<Vec<Edge>> {
    let sql = "SELECT e.from_id, e.to_id, e.kind, f.path, e.line
               FROM edges e LEFT JOIN files f ON f.id = e.file_id
               WHERE e.from_id = ?1 AND e.kind = ?2";
    let mut s = c.prepare_cached(sql).map_err(db_err)?;
    let it = s
        .query_map(params![id, ekind_str(kind)], row_to_edge)
        .map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn edges_to(c: &Connection, id: NodeId, kind: EdgeKind) -> Result<Vec<Edge>> {
    let sql = "SELECT e.from_id, e.to_id, e.kind, f.path, e.line
               FROM edges e LEFT JOIN files f ON f.id = e.file_id
               WHERE e.to_id = ?1 AND e.kind = ?2";
    let mut s = c.prepare_cached(sql).map_err(db_err)?;
    let it = s
        .query_map(params![id, ekind_str(kind)], row_to_edge)
        .map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn edges_from_any(c: &Connection, id: NodeId, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
    edges_any(c, id, kinds, true)
}
pub(crate) fn edges_to_any(c: &Connection, id: NodeId, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
    edges_any(c, id, kinds, false)
}

fn edges_any(c: &Connection, id: NodeId, kinds: &[EdgeKind], from: bool) -> Result<Vec<Edge>> {
    if kinds.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", kinds.len())
        .collect::<Vec<_>>()
        .join(",");
    let col = if from { "from_id" } else { "to_id" };
    let sql = format!(
        "SELECT e.from_id, e.to_id, e.kind, f.path, e.line
         FROM edges e LEFT JOIN files f ON f.id = e.file_id
         WHERE e.{col} = ? AND e.kind IN ({placeholders})"
    );
    let mut s = c.prepare(&sql).map_err(db_err)?;
    let mut p: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(kinds.len() + 1);
    p.push(Box::new(id));
    for k in kinds {
        p.push(Box::new(ekind_str(*k)));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|b| b.as_ref()).collect();
    let it = s.query_map(refs.as_slice(), row_to_edge).map_err(db_err)?;
    let mut out = Vec::new();
    for r in it {
        out.push(r.map_err(db_err)?);
    }
    Ok(out)
}

pub(crate) fn stats(c: &Connection) -> Result<DbStats> {
    let files: i64 = c
        .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
        .map_err(db_err)?;
    let nodes: i64 = c
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .map_err(db_err)?;
    let edges: i64 = c
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
        .map_err(db_err)?;
    let page_count: i64 = c
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .map_err(db_err)?;
    let page_size: i64 = c
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .map_err(db_err)?;
    Ok(DbStats {
        files: files as u64,
        nodes: nodes as u64,
        edges: edges as u64,
        size_bytes: (page_count * page_size) as u64,
        schema_version: schema_version(c)?,
    })
}

fn row_to_file(r: &Row<'_>) -> rusqlite::Result<FileRow> {
    let path: String = r.get(1)?;
    let size: i64 = r.get(4)?;
    Ok(FileRow {
        id: Some(r.get(0)?),
        path: Utf8PathBuf::from(path),
        language: r.get(2)?,
        sha256: r.get(3)?,
        size: size as u64,
        mtime: r.get(5)?,
        indexed_at: r.get(6)?,
    })
}

fn row_to_node(r: &Row<'_>) -> rusqlite::Result<Node> {
    let kind_s: String = r.get(1)?;
    let kind = parse_node_kind(&kind_s).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(BadKind(kind_s.clone())),
        )
    })?;
    let path: String = r.get(4)?;
    Ok(Node {
        id: r.get(0)?,
        kind,
        name: r.get(2)?,
        qualified_name: r.get(3)?,
        file: Utf8PathBuf::from(path),
        start_line: r.get(5)?,
        end_line: r.get(6)?,
        signature: r.get(7)?,
        docstring: r.get(8)?,
        language: r.get(9)?,
    })
}

fn row_to_edge(r: &Row<'_>) -> rusqlite::Result<Edge> {
    let kind_s: String = r.get(2)?;
    let kind = parse_edge_kind(&kind_s).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(BadKind(kind_s.clone())),
        )
    })?;
    let path: Option<String> = r.get(3)?;
    Ok(Edge {
        from: r.get(0)?,
        to: r.get(1)?,
        kind,
        file: path.map(Utf8PathBuf::from),
        line: r.get(4)?,
    })
}

#[derive(Debug)]
struct BadKind(String);
impl std::fmt::Display for BadKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bad kind: {}", self.0)
    }
}
impl std::error::Error for BadKind {}

fn parse_node_kind(s: &str) -> Option<NodeKind> {
    use NodeKind::*;
    Some(match s {
        "file" => File,
        "module" => Module,
        "class" => Class,
        "struct" => Struct,
        "interface" => Interface,
        "trait" => Trait,
        "protocol" => Protocol,
        "function" => Function,
        "method" => Method,
        "property" => Property,
        "field" => Field,
        "variable" => Variable,
        "constant" => Constant,
        "enum" => Enum,
        "enum_member" => EnumMember,
        "type_alias" => TypeAlias,
        "namespace" => Namespace,
        "parameter" => Parameter,
        "import" => Import,
        "export" => Export,
        "route" => Route,
        "component" => Component,
        _ => return None,
    })
}

fn parse_edge_kind(s: &str) -> Option<EdgeKind> {
    use EdgeKind::*;
    Some(match s {
        "contains" => Contains,
        "calls" => Calls,
        "imports" => Imports,
        "exports" => Exports,
        "extends" => Extends,
        "implements" => Implements,
        "references" => References,
        "type_of" => TypeOf,
        "returns" => Returns,
        "instantiates" => Instantiates,
        "overrides" => Overrides,
        "decorates" => Decorates,
        _ => return None,
    })
}

// Suppress unused-warning if Error variant unused elsewhere
#[allow(dead_code)]
fn _check(_: &Error) {}
