use anyhow::Result;
use camino::Utf8PathBuf;
use codegraph_db::Db;
use codegraph_extract::Orchestrator;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebouncedEvent};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

/// Spawn a debounced watcher that re-syncs the workspace on file changes.
/// Runs on a background tokio task; cancellation when the runtime drops.
pub fn spawn(root: Utf8PathBuf, db: Arc<Db>) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = run(root, db) {
            tracing::error!("watcher error: {e}");
        }
    });
}

fn run(root: Utf8PathBuf, db: Arc<Db>) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<DebouncedEvent>>();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: notify_debouncer_full::DebounceEventResult| {
            if let Ok(events) = res {
                let _ = tx.send(events);
            }
        },
    )?;
    debouncer.watch(root.as_std_path(), RecursiveMode::Recursive)?;

    let ignored_dirs = [root.join(crate::CODEGRAPH_DIR), root.join(".git")];
    let mut gitignore_builder = GitignoreBuilder::new(root.as_std_path());
    gitignore_builder.add(root.join(".gitignore"));
    let gitignore = gitignore_builder.build().unwrap_or_else(|_| {
        GitignoreBuilder::new(root.as_std_path())
            .build()
            .expect("empty gitignore builder must build")
    });

    let orch = Orchestrator::with_registry();
    while let Ok(events) = rx.recv() {
        let mut batch = events;
        // Coalesce any batches that arrive while we're about to process one -
        // avoids back-to-back sync passes when the debouncer fires repeatedly
        // in quick succession (e.g. during a large rescan).
        while let Ok(more) = rx.try_recv() {
            batch.extend(more);
        }

        let paths = relevant_paths(&batch, &root, &ignored_dirs, &gitignore);
        if paths.is_empty() {
            continue;
        }
        match orch.sync_paths(&root, &db, &paths) {
            Ok(s) if s.files > 0 => {
                tracing::info!("watch sync: {} files, {} edges", s.files, s.edges)
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("sync failed: {e}"),
        }
    }
    Ok(())
}

fn relevant_paths(
    events: &[DebouncedEvent],
    root: &Utf8PathBuf,
    ignored_dirs: &[Utf8PathBuf],
    gitignore: &Gitignore,
) -> Vec<Utf8PathBuf> {
    let mut out = BTreeSet::new();
    for event in events {
        if event.need_rescan() {
            continue;
        }
        for p in &event.paths {
            if ignored_dirs
                .iter()
                .any(|dir| p.starts_with(dir.as_std_path()))
            {
                continue;
            }
            if gitignore.matched(p, p.is_dir()).is_ignore() {
                continue;
            }
            let Ok(p) = Utf8PathBuf::from_path_buf(p.clone()) else {
                continue;
            };
            if p.starts_with(root) {
                out.insert(p);
            }
        }
    }
    out.into_iter().collect()
}
