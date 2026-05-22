//! Tree-sitter extraction orchestrator + per-language extractors.
//!
//! Native tree-sitter bindings (no WASM). Parallel parse via rayon.

pub mod languages;

// TODO: Orchestrator, file discovery (via `ignore`), parse worker pool,
// per-language extractor trait, standalone extractors (Svelte/Vue/Liquid/DFM).
