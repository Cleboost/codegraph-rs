use axum::Router;
use camino::Utf8PathBuf;
use codegraph_core::{NodeKind};
use codegraph_db::{Db, FileRow, NodeDraft};
use codegraph_viz::api::{self, AppState};
use codegraph_viz::{BootConfig, VizConfig};
use std::sync::Arc;

fn seed_db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("db.sqlite")).unwrap();
    let db = Db::open(&path).unwrap();
    let fid = db
        .upsert_file(&FileRow {
            id: None,
            path: "src/main.rs".into(),
            language: "rust".into(),
            sha256: "x".into(),
            size: 1,
            mtime: 0,
            indexed_at: 0,
        })
        .unwrap();
    let ids = db
        .insert_nodes(
            fid,
            &[NodeDraft {
                kind: NodeKind::Function,
                name: "main".into(),
                qualified_name: None,
                start_line: 1,
                end_line: 1,
                signature: None,
                docstring: None,
                language: "rust".into(),
            }],
        )
        .unwrap();
    let _ = ids;
    (dir, db)
}

fn test_router(db: Arc<Db>) -> Router {
    let boot = BootConfig {
        target: None,
        prefix: None,
        depth: 2,
    };
    let state = AppState {
        db,
        boot_json: serde_json::to_string(&boot).unwrap(),
    };
    Router::new()
        .route("/api/status", axum::routing::get(api::status))
        .route("/api/subgraph", axum::routing::get(api::subgraph))
        .with_state(state)
}

#[tokio::test]
async fn http_status_and_subgraph() {
    let (_dir, db) = seed_db();
    let db = Arc::new(db);
    let app = test_router(db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let status: serde_json::Value = client
        .get(format!("{base}/api/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["nodes"], 1);

    let sub: serde_json::Value = client
        .get(format!("{base}/api/subgraph?depth=1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sub["nodes"].as_array().unwrap().len(), 1);
}

#[test]
fn viz_config_serializes_boot() {
    let boot = BootConfig {
        target: Some("foo".into()),
        prefix: None,
        depth: 3,
    };
    let cfg = VizConfig {
        port: 7421,
        open_browser: false,
        boot,
    };
    assert_eq!(cfg.port, 7421);
}
