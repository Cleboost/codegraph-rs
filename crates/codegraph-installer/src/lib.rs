//! Multi-agent installer. One target per file in `targets/`.
//!
//! Targets: claude, cursor, codex, opencode, hermes.
//! Idempotent install/uninstall, surgical edits preserving user formatting.

pub mod targets;

// TODO: trait AgentTarget { fn name(); fn install(); fn uninstall(); fn detect(); }
// TODO: instructions-template (shared, agent-agnostic).
