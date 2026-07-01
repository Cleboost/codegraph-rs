use crate::Extractor;
use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::sync::Arc;

pub struct FileMatch {
    pub path: Utf8PathBuf,
    pub extractor: Arc<dyn Extractor>,
}

pub type ExtMap = HashMap<&'static str, Arc<dyn Extractor>>;

pub fn build_ext_map(extractors: &[Arc<dyn Extractor>]) -> ExtMap {
    let mut ext_map: ExtMap = HashMap::new();
    for ex in extractors {
        for e in ex.extensions() {
            ext_map.insert(*e, ex.clone());
        }
    }
    ext_map
}

/// Match a single path against the extractor registry, without walking the tree.
/// Used for incremental (watcher-driven) syncs where the caller already knows
/// which paths changed.
pub fn match_extractor(path: &Utf8Path, ext_map: &ExtMap) -> Option<Arc<dyn Extractor>> {
    let ext = path.extension()?;
    ext_map.get(ext).cloned()
}

pub fn walk(root: &Utf8Path, extractors: &[Arc<dyn Extractor>]) -> Vec<FileMatch> {
    let ext_map = build_ext_map(extractors);

    let mut out = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .add_custom_ignore_filename(".codegraphignore")
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ex) = ext_map.get(ext) else {
            continue;
        };
        let Ok(p) = Utf8PathBuf::from_path_buf(path.to_path_buf()) else {
            continue;
        };
        out.push(FileMatch {
            path: p,
            extractor: ex.clone(),
        });
    }
    out
}
