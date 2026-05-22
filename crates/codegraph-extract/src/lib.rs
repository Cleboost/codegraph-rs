//! Tree-sitter extraction orchestrator + per-language extractors.

pub mod languages;
mod orchestrator;
mod walker;

pub use orchestrator::{ExtractStats, Orchestrator};

use codegraph_core::{Error, NodeKind, Result};
use codegraph_db::NodeDraft;
use std::sync::Arc;

/// Local edge using node-indices into the same ExtractResult.nodes vec.
#[derive(Debug, Clone)]
pub struct LocalEdge {
    pub from_idx: usize,
    pub to_idx: usize,
    pub kind: codegraph_core::EdgeKind,
    pub line: Option<u32>,
}

/// Unresolved call site: target is a name; resolved post-pass by name-matcher.
#[derive(Debug, Clone)]
pub struct PendingCall {
    pub from_idx: usize, // index into ExtractResult.nodes
    pub target_name: String,
    pub line: u32,
}

/// Raw import for later resolution by codegraph-resolve.
#[derive(Debug, Clone)]
pub struct RawImport {
    pub from_idx: usize,
    pub module: String,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct ExtractResult {
    pub nodes: Vec<NodeDraft>,
    pub edges: Vec<LocalEdge>,
    pub pending_calls: Vec<PendingCall>,
    pub imports: Vec<RawImport>,
}

pub trait Extractor: Send + Sync {
    fn language(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn ts_language(&self) -> tree_sitter::Language;
    fn extract(&self, source: &str) -> Result<ExtractResult>;
}

pub fn registry() -> Vec<Arc<dyn Extractor>> {
    let mut v: Vec<Arc<dyn Extractor>> = Vec::new();
    #[cfg(feature = "lang-typescript")]
    {
        v.push(Arc::new(languages::typescript::TypeScriptExtractor::new()));
        v.push(Arc::new(languages::typescript::TsxExtractor::new()));
    }
    #[cfg(feature = "lang-javascript")]
    v.push(Arc::new(languages::javascript::JavaScriptExtractor::new()));
    #[cfg(feature = "lang-python")]
    v.push(Arc::new(languages::python::PythonExtractor::new()));
    #[cfg(feature = "lang-rust")]
    v.push(Arc::new(languages::rust::RustExtractor::new()));
    #[cfg(feature = "lang-go")]
    v.push(Arc::new(languages::go::GoExtractor::new()));
    #[cfg(feature = "lang-java")]
    v.push(Arc::new(languages::java::JavaExtractor::new()));
    #[cfg(feature = "lang-c")]
    v.push(Arc::new(languages::c::CExtractor::new()));
    #[cfg(feature = "lang-cpp")]
    v.push(Arc::new(languages::cpp::CppExtractor::new()));
    #[cfg(feature = "lang-csharp")]
    v.push(Arc::new(languages::csharp::CSharpExtractor::new()));
    #[cfg(feature = "lang-ruby")]
    v.push(Arc::new(languages::ruby::RubyExtractor::new()));
    #[cfg(feature = "lang-php")]
    v.push(Arc::new(languages::php::PhpExtractor::new()));
    #[cfg(feature = "lang-scala")]
    v.push(Arc::new(languages::scala::ScalaExtractor::new()));
    #[cfg(feature = "lang-swift")]
    v.push(Arc::new(languages::swift::SwiftExtractor::new()));
    #[cfg(feature = "lang-lua")]
    v.push(Arc::new(languages::lua::LuaExtractor::new()));
    v
}

pub(crate) fn parse_err(s: impl Into<String>) -> Error {
    Error::Parse(s.into())
}

#[allow(dead_code)]
pub(crate) fn _node_kind_smoke() -> NodeKind {
    NodeKind::Function
}
