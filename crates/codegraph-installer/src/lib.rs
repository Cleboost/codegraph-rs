//! Multi-agent installer. One target per file in `targets/`.

pub mod targets;
pub mod bin_install;

use anyhow::Result;
use camino::Utf8PathBuf;
use std::sync::Arc;

pub const INSTRUCTIONS_MD: &str = include_str!("instructions-template.md");

#[derive(Debug, Clone)]
pub struct InstallOpts {
    /// Workspace root (for project-scoped installs).
    pub project_root: Option<Utf8PathBuf>,
    /// Install globally (in user home) rather than project-local.
    pub global: bool,
    /// Absolute path to the `codegraph` binary (for MCP `command`).
    pub binary_path: Utf8PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectStatus {
    NotFound,
    Found,
    AlreadyConfigured,
}

#[derive(Debug, Clone)]
pub enum InstallReport {
    Installed(Vec<Utf8PathBuf>),
    Unchanged,
    Updated(Vec<Utf8PathBuf>),
    Skipped(String),
}

pub trait AgentTarget: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn detect(&self, opts: &InstallOpts) -> DetectStatus;
    fn install(&self, opts: &InstallOpts) -> Result<InstallReport>;
    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport>;
}

pub fn registry() -> Vec<Arc<dyn AgentTarget>> {
    vec![
        Arc::new(targets::claude::ClaudeTarget),
        Arc::new(targets::cursor::CursorTarget),
        Arc::new(targets::codex::CodexTarget),
        Arc::new(targets::opencode::OpencodeTarget),
        Arc::new(targets::hermes::HermesTarget),
    ]
}

/// Project-scoped targets only (skip ones that only support global config).
pub fn project_registry() -> Vec<Arc<dyn AgentTarget>> {
    registry()
        .into_iter()
        .filter(|t| t.id() != "codex")
        .collect()
}
