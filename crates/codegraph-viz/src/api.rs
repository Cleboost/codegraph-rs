use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use codegraph_api::GraphApi;
use codegraph_core::EdgeKind;
use codegraph_db::Db;
use codegraph_graph::{SubgraphRequest, VIZ_EDGE_KINDS};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub boot_json: String,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: u32,
}

fn default_search_limit() -> u32 {
    20
}

#[derive(Deserialize)]
pub struct SubgraphParams {
    pub seed: Option<i64>,
    pub query: Option<String>,
    pub prefix: Option<String>,
    #[serde(default = "default_depth")]
    pub depth: u32,
    pub kinds: Option<String>,
    pub limit: Option<u32>,
}

fn default_depth() -> u32 {
    2
}

#[derive(Deserialize)]
pub struct NeighborParams {
    #[serde(default = "default_depth")]
    pub depth: u32,
    pub kinds: Option<String>,
}

#[derive(Deserialize)]
pub struct DepthParams {
    #[serde(default = "default_depth")]
    pub depth: u32,
}

#[derive(Deserialize)]
pub struct FilesParams {
    pub prefix: Option<String>,
}

pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    match api.stats() {
        Ok(s) => Json(s).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    match api.search(&params.q, params.limit) {
        Ok(hits) => Json(hits).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn node(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    match api.node_by_id(id) {
        Ok(Some(n)) => Json(n).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "node not found" })),
        )
            .into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn subgraph(
    State(state): State<AppState>,
    Query(params): Query<SubgraphParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    let kinds = parse_kinds(params.kinds.as_deref());
    let req = SubgraphRequest {
        seed: params.seed,
        query: params.query,
        prefix: params.prefix,
        depth: params.depth,
        kinds,
        node_limit: params.limit,
        edge_limit: params.limit.map(|l| l.saturating_mul(2)),
    };
    match api.subgraph(req) {
        Ok(s) => Json(s).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn neighbors(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<NeighborParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    let kinds = parse_kinds(params.kinds.as_deref());
    match api.neighborhood(id, params.depth, &kinds) {
        Ok(h) => Json(h).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn files(
    State(state): State<AppState>,
    Query(params): Query<FilesParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    let prefix = params.prefix.unwrap_or_default();
    match api.files(&prefix) {
        Ok(f) => Json(f).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn callers(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<DepthParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    match api.callers(id, params.depth) {
        Ok(h) => Json(h).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn callees(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<DepthParams>,
) -> impl IntoResponse {
    let api = GraphApi::new(&state.db);
    match api.callees(id, params.depth) {
        Ok(h) => Json(h).into_response(),
        Err(e) => api_error(e),
    }
}

pub async fn boot(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        state.boot_json,
    )
}

fn parse_kinds(raw: Option<&str>) -> Vec<EdgeKind> {
    let Some(raw) = raw else {
        return VIZ_EDGE_KINDS.to_vec();
    };
    let all = [
        EdgeKind::Contains,
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::Exports,
        EdgeKind::Extends,
        EdgeKind::Implements,
        EdgeKind::References,
        EdgeKind::TypeOf,
        EdgeKind::Returns,
        EdgeKind::Instantiates,
        EdgeKind::Overrides,
        EdgeKind::Decorates,
    ];
    let mut kinds = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(k) = all.iter().find(|k| k.as_str() == part) {
            kinds.push(*k);
        }
    }
    if kinds.is_empty() {
        VIZ_EDGE_KINDS.to_vec()
    } else {
        kinds
    }
}

fn api_error(e: codegraph_core::Error) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}
