use crate::{walker, Extractor, ExtractResult};
use camino::Utf8Path;
use codegraph_core::Result;
use codegraph_db::{Db, EdgeDraft, FileRow, NodeDraft};
use codegraph_resolve::{PendingCallRow, Resolver};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Default, Clone)]
pub struct ExtractStats {
    pub files: u64,
    pub nodes: u64,
    pub edges: u64,
    pub skipped: u64,
    pub resolved_calls: u64,
}

pub struct Orchestrator {
    extractors: Vec<Arc<dyn Extractor>>,
}

impl Orchestrator {
    pub fn new(extractors: Vec<Arc<dyn Extractor>>) -> Self { Self { extractors } }

    pub fn with_registry() -> Self { Self::new(crate::registry()) }

    pub fn index_all(&self, root: &Utf8Path, db: &Db) -> Result<ExtractStats> {
        db.purge()?;
        self.sync(root, db)
    }

    pub fn sync(&self, root: &Utf8Path, db: &Db) -> Result<ExtractStats> {
        let files = walker::walk(root, &self.extractors);
        let parsed: Vec<_> = files
            .par_iter()
            .filter_map(|fm| parse_one(fm).ok().flatten())
            .collect();

        let mut stats = ExtractStats::default();
        let mut all_pending: Vec<PendingCallRow> = Vec::new();
        for Parsed { row, result, ext_idx: _ } in parsed {
            // Skip if file's existing sha matches — no-op sync optimization.
            if let Ok(Some(existing)) = db.file_by_path(row.path.as_str()) {
                if existing.sha256 == row.sha256 {
                    stats.skipped += 1;
                    continue;
                }
                if let Some(eid) = existing.id { db.delete_file_cascade(eid)?; }
            }

            let fid = db.upsert_file(&row)?;
            let drafts: Vec<NodeDraft> = result.nodes;
            let ids = db.insert_nodes(fid, &drafts)?;
            let edges: Vec<EdgeDraft> = result
                .edges
                .into_iter()
                .filter_map(|e| {
                    let f = *ids.get(e.from_idx)?;
                    let t = *ids.get(e.to_idx)?;
                    Some(EdgeDraft {
                        from_id: f, to_id: t, kind: e.kind,
                        file_id: Some(fid), line: e.line, source: Some("extract".into()),
                    })
                })
                .collect();
            stats.nodes += ids.len() as u64;
            stats.edges += edges.len() as u64;
            db.insert_edges(&edges)?;
            // Translate pending_calls (local node indices) into resolver rows.
            for pc in &result.pending_calls {
                if let Some(from_id) = ids.get(pc.from_idx) {
                    all_pending.push(PendingCallRow {
                        from_id: *from_id,
                        target_name: pc.target_name.clone(),
                        file_id: fid,
                        line: pc.line,
                    });
                }
            }
            stats.files += 1;
        }
        let resolved = Resolver::new(db).resolve_calls(&all_pending)?;
        stats.resolved_calls = resolved as u64;
        stats.edges += resolved as u64;
        Ok(stats)
    }
}

struct Parsed {
    row: FileRow,
    result: ExtractResult,
    ext_idx: usize,
}

fn parse_one(fm: &walker::FileMatch) -> Result<Option<Parsed>> {
    let bytes = match std::fs::read(fm.path.as_std_path()) {
        Ok(b) if b.len() < 4 * 1024 * 1024 => b,
        _ => return Ok(None),
    };
    let source = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let mut h = Sha256::new();
    h.update(&bytes);
    let sha = hex::encode(h.finalize());
    let meta = std::fs::metadata(fm.path.as_std_path())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let result = fm.extractor.extract(source)?;
    let row = FileRow {
        id: None,
        path: fm.path.clone(),
        language: fm.extractor.language().to_string(),
        sha256: sha,
        size: bytes.len() as u64,
        mtime,
        indexed_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };
    Ok(Some(Parsed { row, result, ext_idx: 0 }))
}
