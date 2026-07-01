use anyhow::Result;
use camino::Utf8PathBuf;
use codegraph_db::Db;
use codegraph_extract::Orchestrator;
use ignore::gitignore::GitignoreBuilder;
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
        let relevant = events.iter().any(|event| {
            event.paths.iter().any(|p| {
                let under_ignored_dir = ignored_dirs.iter().any(|dir| p.starts_with(dir.as_std_path()));
                if under_ignored_dir {
                    return false;
                }
                !gitignore.matched(p, p.is_dir()).is_ignore()
            })
        });
        if !relevant {
            continue;
        }
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
