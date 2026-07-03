use camino::Utf8PathBuf;
use codegraph_db::Db;
use codegraph_extract::Orchestrator;

fn open() -> (tempfile::TempDir, Db) {
    let d = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(d.path().join("db.sqlite")).unwrap();
    let db = Db::open(&p).unwrap();
    (d, db)
}

#[test]
fn index_fixtures_dir() {
    let (_keep, db) = open();
    let orch = Orchestrator::with_registry();
    let root = Utf8PathBuf::from_path_buf(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
    )
    .unwrap();

    let stats = orch.index_all(&root, &db).unwrap();
    assert!(
        stats.files >= 7,
        "expected at least 7 files, got {}",
        stats.files
    );
    assert!(stats.nodes > 0);

    // Java
    let hits = db.search_nodes("UserService", 10).unwrap();
    assert!(
        hits.iter().any(|n| n.language == "java"),
        "expected java hit"
    );

    // Ruby
    let hits = db.search_nodes("UserService", 10).unwrap();
    assert!(
        hits.iter().any(|n| n.language == "ruby"),
        "expected ruby hit"
    );

    // Python
    let hits = db.search_nodes("process_user", 10).unwrap();
    assert!(
        hits.iter().any(|n| n.language == "python"),
        "expected python hit"
    );

    // Go
    let hits = db.search_nodes("ProcessUser", 10).unwrap();
    assert!(hits.iter().any(|n| n.language == "go"), "expected go hit");

    // JS
    let hits = db.search_nodes("processUser", 10).unwrap();
    assert!(
        hits.iter().any(|n| n.language == "javascript"),
        "expected js hit"
    );

    // TS-specific: should have processUser
    let hits = db.search_nodes("processUser", 10).unwrap();
    assert!(
        hits.iter().any(|n| n.name == "processUser"),
        "missing processUser in {:?}",
        hits
    );

    // Rust-specific: should have process_user
    let hits = db.search_nodes("process_user", 10).unwrap();
    assert!(hits.iter().any(|n| n.name == "process_user"));

    // UserService should appear (TS class + Rust struct)
    let hits = db.search_nodes("UserService", 10).unwrap();
    assert!(
        hits.len() >= 2,
        "expected UserService from both TS and Rust, got {}",
        hits.len()
    );
}

#[test]
fn sync_skips_unchanged() {
    let (_keep, db) = open();
    let orch = Orchestrator::with_registry();
    let root = Utf8PathBuf::from_path_buf(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
    )
    .unwrap();

    orch.index_all(&root, &db).unwrap();
    let s2 = orch.sync(&root, &db).unwrap();
    assert_eq!(s2.files, 0, "no new files should be indexed");
    assert!(s2.skipped >= 2);
}

#[test]
fn sync_paths_skips_mtime_only_touch() {
    use std::time::{Duration, SystemTime};

    let (_keep, db) = open();
    let orch = Orchestrator::with_registry();
    let fixture = Utf8PathBuf::from_path_buf(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.rs"),
    )
    .unwrap();

    orch.sync_paths(&fixture.parent().unwrap(), &db, std::slice::from_ref(&fixture))
        .unwrap();
    let indexed = db.stats().unwrap().files;
    assert!(indexed >= 1, "fixture should be indexed");

    let later = SystemTime::now() + Duration::from_secs(5);
    filetime::set_file_mtime(
        fixture.as_std_path(),
        filetime::FileTime::from_system_time(later),
    )
    .unwrap();

    let stats = orch
        .sync_paths(&fixture.parent().unwrap(), &db, std::slice::from_ref(&fixture))
        .unwrap();
    assert_eq!(stats.files, 0, "mtime-only touch must not re-index");
    assert!(stats.skipped >= 1, "expected skip, got {:?}", stats);
}
