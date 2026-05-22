use camino::Utf8PathBuf;
use codegraph_core::{EdgeKind, NodeKind};
use codegraph_db::{Db, EdgeDraft, FileRow, NodeDraft};
use codegraph_graph::Traversal;

fn db() -> (tempfile::TempDir, Db) {
    let d = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(d.path().join("db.sqlite")).unwrap();
    (d, Db::open(&p).unwrap())
}

fn mk_file(db: &Db, p: &str) -> i64 {
    db.upsert_file(&FileRow {
        id: None,
        path: p.into(),
        language: "test".into(),
        sha256: "x".into(),
        size: 0,
        mtime: 0,
        indexed_at: 0,
    })
    .unwrap()
}

fn node(name: &str) -> NodeDraft {
    NodeDraft {
        kind: NodeKind::Function,
        name: name.into(),
        qualified_name: None,
        start_line: 1,
        end_line: 1,
        signature: None,
        docstring: None,
        language: "test".into(),
    }
}

#[test]
fn callers_callees_chain() {
    // A -> B -> C -> D
    let (_d, db) = db();
    let f = mk_file(&db, "a.ts");
    let ids = db
        .insert_nodes(f, &[node("a"), node("b"), node("c"), node("d")])
        .unwrap();
    let calls = |from: usize, to: usize| EdgeDraft {
        from_id: ids[from],
        to_id: ids[to],
        kind: EdgeKind::Calls,
        file_id: Some(f),
        line: None,
        source: None,
    };
    db.insert_edges(&[calls(0, 1), calls(1, 2), calls(2, 3)])
        .unwrap();

    let t = Traversal::new(&db);
    let cees = t.callees(ids[0], 3).unwrap();
    assert_eq!(cees.nodes.len(), 3);
    assert!(cees.nodes.iter().any(|n| n.name == "d"));

    let cers = t.callers(ids[3], 3).unwrap();
    assert_eq!(cers.nodes.len(), 3);
    assert!(cers.nodes.iter().any(|n| n.name == "a"));

    // depth limit
    let cees2 = t.callees(ids[0], 1).unwrap();
    assert_eq!(cees2.nodes.len(), 1);
    assert_eq!(cees2.nodes[0].name, "b");
}

#[test]
fn impact_groups_by_depth() {
    let (_d, db) = db();
    let f = mk_file(&db, "a.ts");
    let ids = db
        .insert_nodes(f, &[node("root"), node("d1"), node("d2"), node("d2b")])
        .unwrap();
    // d1 -> root, d2 -> d1, d2b -> d1
    db.insert_edges(&[
        EdgeDraft {
            from_id: ids[1],
            to_id: ids[0],
            kind: EdgeKind::Calls,
            file_id: Some(f),
            line: None,
            source: None,
        },
        EdgeDraft {
            from_id: ids[2],
            to_id: ids[1],
            kind: EdgeKind::Calls,
            file_id: Some(f),
            line: None,
            source: None,
        },
        EdgeDraft {
            from_id: ids[3],
            to_id: ids[1],
            kind: EdgeKind::Calls,
            file_id: Some(f),
            line: None,
            source: None,
        },
    ])
    .unwrap();
    let t = Traversal::new(&db);
    let imp = t.impact_radius(ids[0], 3).unwrap();
    assert_eq!(imp.direct.len(), 1);
    assert_eq!(imp.transitive.len(), 2);
    assert!(!imp.truncated);
}
