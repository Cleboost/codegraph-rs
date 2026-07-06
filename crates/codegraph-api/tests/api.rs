use camino::Utf8PathBuf;
use codegraph_api::GraphApi;
use codegraph_core::{EdgeKind, NodeKind};
use codegraph_db::{Db, EdgeDraft, FileRow, NodeDraft};
use codegraph_graph::{SubgraphRequest, Traversal};

fn tmp_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("db.sqlite")).unwrap();
    let db = Db::open(&path).unwrap();
    (dir, db)
}

fn seed_graph(db: &Db) -> (i64, i64, i64) {
    let fid = db
        .upsert_file(&FileRow {
            id: None,
            path: "src/a.ts".into(),
            language: "typescript".into(),
            sha256: "x".into(),
            size: 1,
            mtime: 0,
            indexed_at: 0,
        })
        .unwrap();
    let ids = db
        .insert_nodes(
            fid,
            &[
                NodeDraft {
                    kind: NodeKind::Function,
                    name: "caller".into(),
                    qualified_name: None,
                    start_line: 1,
                    end_line: 2,
                    signature: None,
                    docstring: None,
                    language: "typescript".into(),
                },
                NodeDraft {
                    kind: NodeKind::Function,
                    name: "callee".into(),
                    qualified_name: None,
                    start_line: 3,
                    end_line: 4,
                    signature: None,
                    docstring: None,
                    language: "typescript".into(),
                },
            ],
        )
        .unwrap();
    db.insert_edges(&[EdgeDraft {
        from_id: ids[0],
        to_id: ids[1],
        kind: EdgeKind::Calls,
        file_id: Some(fid),
        line: Some(1),
        source: None,
    }])
    .unwrap();
    (ids[0], ids[1], fid)
}

#[test]
fn subgraph_by_seed() {
    let (_d, db) = tmp_db();
    let (caller, callee, _) = seed_graph(&db);
    let api = GraphApi::new(&db);
    let sub = api
        .subgraph(SubgraphRequest {
            seed: Some(caller),
            query: None,
            prefix: None,
            depth: 2,
            kinds: vec![EdgeKind::Calls],
            node_limit: None,
            edge_limit: None,
        })
        .unwrap();
    assert!(sub.seed.is_some());
    assert!(sub.nodes.iter().any(|n| n.id == callee));
    assert!(!sub.edges.is_empty());
}

#[test]
fn subgraph_by_query() {
    let (_d, db) = tmp_db();
    seed_graph(&db);
    let api = GraphApi::new(&db);
    let sub = api
        .subgraph(SubgraphRequest {
            seed: None,
            query: Some("caller".into()),
            prefix: None,
            depth: 1,
            kinds: vec![EdgeKind::Calls],
            node_limit: None,
            edge_limit: None,
        })
        .unwrap();
    assert_eq!(sub.seed.as_ref().map(|n| n.name.as_str()), Some("caller"));
}

#[test]
fn subgraph_default_overview() {
    let (_d, db) = tmp_db();
    seed_graph(&db);
    let api = GraphApi::new(&db);
    let sub = api
        .subgraph(SubgraphRequest {
            seed: None,
            query: None,
            prefix: None,
            depth: 2,
            kinds: vec![EdgeKind::Calls],
            node_limit: None,
            edge_limit: None,
        })
        .unwrap();
    assert_eq!(sub.nodes.len(), 2);
    assert!(!sub.edges.is_empty());
}

#[test]
fn neighborhood_bidirectional() {
    let (_d, db) = tmp_db();
    let (caller, callee, _) = seed_graph(&db);
    let hits = Traversal::new(&db)
        .neighborhood(callee, 1, &[EdgeKind::Calls])
        .unwrap();
    assert!(hits.nodes.iter().any(|n| n.id == caller));
}
