use crate::config::ExtractConfig;
use crate::{walker, ExtractResult, Extractor};
use camino::{Utf8Path, Utf8PathBuf};
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
    pub fn new(extractors: Vec<Arc<dyn Extractor>>) -> Self {
        Self { extractors }
    }

    pub fn with_registry() -> Self {
        Self::new(crate::registry())
    }

    pub fn index_all(&self, root: &Utf8Path, db: &Db) -> Result<ExtractStats> {
        db.purge()?;
        self.sync(root, db)
    }

    pub fn sync(&self, root: &Utf8Path, db: &Db) -> Result<ExtractStats> {
        let config = ExtractConfig::load(root);
        let files = walker::walk(root, &self.extractors, &config);
        let results: Vec<_> = files.par_iter().map(|fm| parse_one(fm, db)).collect();
        let mut parsed = Vec::with_capacity(results.len());
        let mut skipped = 0u64;
        for r in results {
            match r {
                Ok(None) => skipped += 1,
                Ok(Some(p)) => parsed.push(p),
                Err(_) => {}
            }
        }
        let mut stats = self.apply(db, parsed)?;
        stats.skipped += skipped;
        Ok(stats)
    }

    /// Sync only the given paths instead of walking the whole tree. Used by the
    /// watcher so that a burst of filesystem events costs O(changed files),
    /// not O(repo size).
    pub fn sync_paths(&self, root: &Utf8Path, db: &Db, paths: &[Utf8PathBuf]) -> Result<ExtractStats> {
        let config = ExtractConfig::load(root);
        let ext_map = walker::build_ext_map(&self.extractors);
        let opts = walker::walk_options(&self.extractors, &config, root);
        let mut matches = Vec::new();
        for p in paths {
            if !p.as_std_path().is_file() {
                // Deleted (or not a regular file): drop it from the index if present.
                if let Ok(Some(existing)) = db.file_by_path(p.as_str()) {
                    if let Some(eid) = existing.id {
                        db.delete_file_cascade(eid)?;
                    }
                }
                continue;
            }
            if let Some(extractor) = walker::match_extractor(p, &ext_map, &opts) {
                matches.push(walker::FileMatch {
                    path: p.clone(),
                    extractor,
                });
            }
        }
        let results: Vec<_> = matches.par_iter().map(|fm| parse_one(fm, db)).collect();
        let mut parsed = Vec::with_capacity(results.len());
        let mut skipped = 0u64;
        for r in results {
            match r {
                Ok(None) => skipped += 1,
                Ok(Some(p)) => parsed.push(p),
                Err(_) => {}
            }
        }
        let mut apply_stats = self.apply(db, parsed)?;
        apply_stats.skipped += skipped;
        Ok(apply_stats)
    }

    fn apply(&self, db: &Db, parsed: Vec<Parsed>) -> Result<ExtractStats> {
        let mut stats = ExtractStats::default();
        let mut all_pending: Vec<PendingCallRow> = Vec::new();
        for Parsed { row, result } in parsed {
            // Skip if file's existing sha matches — no-op sync optimization.
            if let Ok(Some(existing)) = db.file_by_path(row.path.as_str()) {
                if existing.sha256 == row.sha256 {
                    stats.skipped += 1;
                    continue;
                }
                if let Some(eid) = existing.id {
                    db.delete_file_cascade(eid)?;
                }
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
                        from_id: f,
                        to_id: t,
                        kind: e.kind,
                        file_id: Some(fid),
                        line: e.line,
                        source: Some("extract".into()),
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
}

fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parse a single file if its content changed since the last index.
///
/// Watcher-driven syncs often see mtime-only updates (IDE saves, `touch`, …)
/// with identical bytes. Skipping tree-sitter when metadata or sha256 match
/// avoids sustained multi-core CPU during large no-op batches.
fn parse_one(fm: &walker::FileMatch, db: &Db) -> Result<Option<Parsed>> {
    let meta = std::fs::metadata(fm.path.as_std_path())?;
    let mtime = file_mtime(&meta);
    let size = meta.len();

    if let Ok(Some(existing)) = db.file_by_path(fm.path.as_str()) {
        if existing.mtime == mtime && existing.size == size {
            return Ok(None);
        }
    }

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

    if let Ok(Some(existing)) = db.file_by_path(fm.path.as_str()) {
        if existing.sha256 == sha {
            if existing.mtime != mtime || existing.size != size {
                db.update_file_metadata(fm.path.as_str(), mtime, size)?;
            }
            return Ok(None);
        }
    }

    let result = fm.extractor.extract(source)?;
    let row = FileRow {
        id: None,
        path: fm.path.clone(),
        language: fm.extractor.language().to_string(),
        sha256: sha,
        size: size as u64,
        mtime,
        indexed_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };
    Ok(Some(Parsed { row, result }))
}
