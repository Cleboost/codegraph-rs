use camino::Utf8PathBuf;
use codegraph_core::{EdgeKind, NodeKind};
use codegraph_db::{Db, EdgeDraft, FileRow, NodeDraft, SCHEMA_VERSION};

fn tmp_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("db.sqlite")).unwrap();
    let db = Db::open(&path).unwrap();
    (dir, db)
}

fn mk_file(path: &str) -> FileRow {
    FileRow {
        id: None,
        path: path.into(),
        language: "typescript".into(),
        sha256: "deadbeef".into(),
        size: 100,
        mtime: 0,
        indexed_at: 0,
    }
}

fn mk_node(name: &str, kind: NodeKind) -> NodeDraft {
    NodeDraft {
        kind,
        name: name.into(),
        qualified_name: Some(format!("mod::{name}")),
        start_line: 1,
        end_line: 10,
        signature: Some(format!("fn {name}()")),
        docstring: None,
        language: "typescript".into(),
    }
}

#[test]
fn schema_version_set() {
    let (_d, db) = tmp_db();
    assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn upsert_file_idempotent() {
    let (_d, db) = tmp_db();
    let id1 = db.upsert_file(&mk_file("src/foo.ts")).unwrap();
    let id2 = db.upsert_file(&mk_file("src/foo.ts")).unwrap();
    assert_eq!(id1, id2);
    assert_eq!(db.stats().unwrap().files, 1);
}

#[test]
fn nodes_edges_roundtrip() {
    let (_d, db) = tmp_db();
    let fid = db.upsert_file(&mk_file("src/a.ts")).unwrap();
    let ids = db
        .insert_nodes(
            fid,
            &[
                mk_node("foo", NodeKind::Function),
                mk_node("bar", NodeKind::Function),
            ],
        )
        .unwrap();
    assert_eq!(ids.len(), 2);

    db.insert_edges(&[EdgeDraft {
        from_id: ids[0],
        to_id: ids[1],
        kind: EdgeKind::Calls,
        file_id: Some(fid),
        line: Some(5),
        source: None,
    }])
    .unwrap();

    let callees = db.callees_of(ids[0]).unwrap();
    assert_eq!(callees.len(), 1);
    assert_eq!(callees[0].to, ids[1]);

    let callers = db.callers_of(ids[1]).unwrap();
    assert_eq!(callers.len(), 1);

    let stats = db.stats().unwrap();
    assert_eq!(stats.files, 1);
    assert_eq!(stats.nodes, 2);
    assert_eq!(stats.edges, 1);
}

#[test]
fn fts_search() {
    let (_d, db) = tmp_db();
    let fid = db.upsert_file(&mk_file("src/a.ts")).unwrap();
    db.insert_nodes(
        fid,
        &[
            mk_node("processUser", NodeKind::Function),
            mk_node("formatEmail", NodeKind::Function),
            mk_node("randomThing", NodeKind::Variable),
        ],
    )
    .unwrap();

    let hits = db.search_nodes("process", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "processUser");

    let hits = db.search_nodes("format", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "formatEmail");
}

#[test]
fn delete_cascade() {
    let (_d, db) = tmp_db();
    let fid = db.upsert_file(&mk_file("src/a.ts")).unwrap();
    let ids = db
        .insert_nodes(fid, &[mk_node("foo", NodeKind::Function)])
        .unwrap();
    db.insert_edges(&[EdgeDraft {
        from_id: ids[0],
        to_id: ids[0],
        kind: EdgeKind::Calls,
        file_id: Some(fid),
        line: None,
        source: None,
    }])
    .unwrap();

    db.delete_file_cascade(fid).unwrap();
    let s = db.stats().unwrap();
    assert_eq!(s.files, 0);
    assert_eq!(s.nodes, 0);
    assert_eq!(s.edges, 0);
}

#[test]
fn nodes_by_name_returns_all() {
    let (_d, db) = tmp_db();
    let fid = db.upsert_file(&mk_file("src/a.ts")).unwrap();
    db.insert_nodes(
        fid,
        &[
            mk_node("foo", NodeKind::Function),
            mk_node("foo", NodeKind::Variable),
        ],
    )
    .unwrap();
    assert_eq!(db.nodes_by_name("foo").unwrap().len(), 2);
}

#[test]
fn files_under_resolves_relative_prefix_against_workspace_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let db_path = root.join(".codegraph").join("db.sqlite");
    let db = Db::open(&db_path).unwrap();

    let file_path = root.join("src/foo.ts");
    db.upsert_file(&FileRow {
        id: None,
        path: file_path.clone(),
        language: "typescript".into(),
        sha256: "deadbeef".into(),
        size: 100,
        mtime: 0,
        indexed_at: 0,
    })
    .unwrap();
    db.upsert_file(&FileRow {
        id: None,
        path: root.join("lib/bar.ts"),
        language: "typescript".into(),
        sha256: "cafebabe".into(),
        size: 100,
        mtime: 0,
        indexed_at: 0,
    })
    .unwrap();

    assert_eq!(db.files_under("./src/").unwrap().len(), 1);
    assert_eq!(db.files_under("src").unwrap().len(), 1);
    assert_eq!(
        db.files_under(file_path.as_str()).unwrap()[0].path,
        file_path
    );
    assert_eq!(db.files_under("lib").unwrap().len(), 1);
    assert_eq!(db.files_under("").unwrap().len(), 2);
}

#[test]
fn files_under_matches_backslash_stored_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let db_path = root.join(".codegraph").join("db.sqlite");
    let db = Db::open(&db_path).unwrap();

    let stored = format!("{}\\src\\foo.ts", root);
    db.upsert_file(&FileRow {
        id: None,
        path: stored.clone().into(),
        language: "typescript".into(),
        sha256: "deadbeef".into(),
        size: 100,
        mtime: 0,
        indexed_at: 0,
    })
    .unwrap();

    assert_eq!(db.files_under("src").unwrap().len(), 1);
    assert_eq!(db.files_under("./src/").unwrap().len(), 1);
    assert_eq!(
        db.files_under("src/foo.ts").unwrap()[0].path.as_str(),
        stored
    );
}

#[test]
fn nodes_under_prefix_and_edges_between() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let db_path = root.join(".codegraph").join("db.sqlite");
    let db = Db::open(&db_path).unwrap();

    let fid_a = db
        .upsert_file(&FileRow {
            id: None,
            path: root.join("src/a.ts"),
            language: "typescript".into(),
            sha256: "a".into(),
            size: 1,
            mtime: 0,
            indexed_at: 0,
        })
        .unwrap();
    let fid_b = db
        .upsert_file(&FileRow {
            id: None,
            path: root.join("lib/b.ts"),
            language: "typescript".into(),
            sha256: "b".into(),
            size: 1,
            mtime: 0,
            indexed_at: 0,
        })
        .unwrap();

    let ids_a = db
        .insert_nodes(
            fid_a,
            &[
                mk_node("foo", NodeKind::Function),
                mk_node("bar", NodeKind::Function),
            ],
        )
        .unwrap();
    let ids_b = db
        .insert_nodes(fid_b, &[mk_node("baz", NodeKind::Function)])
        .unwrap();

    db.insert_edges(&[
        EdgeDraft {
            from_id: ids_a[0],
            to_id: ids_a[1],
            kind: EdgeKind::Calls,
            file_id: Some(fid_a),
            line: Some(1),
            source: None,
        },
        EdgeDraft {
            from_id: ids_a[0],
            to_id: ids_b[0],
            kind: EdgeKind::Calls,
            file_id: Some(fid_a),
            line: Some(2),
            source: None,
        },
    ])
    .unwrap();

    let under_src = db.nodes_under_prefix("src", 100).unwrap();
    assert_eq!(under_src.len(), 2);

    let by_files = db.nodes_by_file_ids(&[fid_a], 100).unwrap();
    assert_eq!(by_files.len(), 2);

    let node_ids: Vec<i64> = under_src.iter().map(|n| n.id).collect();
    let internal = db
        .edges_between(&node_ids, &[EdgeKind::Calls], 100)
        .unwrap();
    assert_eq!(internal.len(), 1);
    assert_eq!(internal[0].from, ids_a[0]);
    assert_eq!(internal[0].to, ids_a[1]);
}
