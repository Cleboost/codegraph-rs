use anyhow::Result;
use camino::Utf8PathBuf;
use codegraph_db::Db;
use codegraph_extract::Orchestrator;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebouncedEvent};
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

    let orch = Orchestrator::with_registry();
    while let Ok(_events) = rx.recv() {
        match orch.sync(&root, &db) {
            Ok(s) if s.files > 0 => {
                tracing::info!("watch sync: {} files, {} edges", s.files, s.edges)
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("sync failed: {e}"),
        }
    }
    Ok(())
}
