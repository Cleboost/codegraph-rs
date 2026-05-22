//! Codex CLI — config at `~/.codex/config.toml`, instructions at `~/.codex/AGENTS.md`.
//! Uses toml_edit to preserve sibling tables and user formatting.

use crate::{AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD};
use anyhow::Result;
use camino::Utf8PathBuf;
use toml_edit::{value, Array, DocumentMut, Item, Table};

pub struct CodexTarget;

impl CodexTarget {
    fn config_path(&self) -> Option<Utf8PathBuf> {
        let home = dirs::home_dir()?;
        Utf8PathBuf::from_path_buf(home.join(".codex").join("config.toml")).ok()
    }
    fn agents_path(&self) -> Option<Utf8PathBuf> {
        let home = dirs::home_dir()?;
        Utf8PathBuf::from_path_buf(home.join(".codex").join("AGENTS.md")).ok()
    }
}

impl AgentTarget for CodexTarget {
    fn id(&self) -> &'static str { "codex" }
    fn label(&self) -> &'static str { "Codex CLI" }

    fn detect(&self, _opts: &InstallOpts) -> DetectStatus {
        let Some(p) = self.config_path() else { return DetectStatus::NotFound };
        if !p.exists() { return DetectStatus::NotFound; }
        let Ok(text) = std::fs::read_to_string(p.as_std_path()) else { return DetectStatus::Found };
        let Ok(doc) = text.parse::<DocumentMut>() else { return DetectStatus::Found };
        if doc.get("mcp_servers").and_then(|v| v.as_table()).and_then(|t| t.get("codegraph")).is_some() {
            DetectStatus::AlreadyConfigured
        } else {
            DetectStatus::Found
        }
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let config = self.config_path().ok_or_else(|| anyhow::anyhow!("no codex config path"))?;
        let text = std::fs::read_to_string(config.as_std_path()).unwrap_or_default();
        let mut doc: DocumentMut = if text.is_empty() { DocumentMut::new() } else { text.parse()? };

        let mut servers = match doc.remove("mcp_servers") {
            Some(Item::Table(t)) => t,
            _ => {
                let mut t = Table::new();
                t.set_implicit(true);
                t
            }
        };

        let mut cg = Table::new();
        cg["command"] = value(opts.binary_path.as_str());
        let mut args = Array::new();
        args.push("serve");
        args.push("--mcp");
        if let Some(root) = &opts.project_root {
            args.push("--path");
            args.push(root.as_str());
        }
        cg["args"] = value(args);

        let existing = servers.get("codegraph").map(|i| i.to_string());
        let new_str = Item::Table(cg.clone()).to_string();
        let changed = existing.as_deref() != Some(new_str.as_str());

        servers["codegraph"] = Item::Table(cg);
        doc["mcp_servers"] = Item::Table(servers);

        let mut written = Vec::new();
        if changed {
            if let Some(parent) = config.parent() { std::fs::create_dir_all(parent.as_std_path())?; }
            std::fs::write(config.as_std_path(), doc.to_string())?;
            written.push(config);
        }
        if let Some(md) = self.agents_path() {
            let existing = std::fs::read_to_string(md.as_std_path()).ok();
            if existing.as_deref() != Some(INSTRUCTIONS_MD) {
                if let Some(parent) = md.parent() { std::fs::create_dir_all(parent.as_std_path())?; }
                std::fs::write(md.as_std_path(), INSTRUCTIONS_MD)?;
                written.push(md);
            }
        }
        if written.is_empty() { Ok(InstallReport::Unchanged) } else { Ok(InstallReport::Installed(written)) }
    }

    fn uninstall(&self, _opts: &InstallOpts) -> Result<InstallReport> {
        let Some(config) = self.config_path() else { return Ok(InstallReport::Unchanged) };
        if !config.exists() { return Ok(InstallReport::Unchanged); }
        let text = std::fs::read_to_string(config.as_std_path())?;
        let mut doc: DocumentMut = text.parse()?;
        let mut changed = false;
        if let Some(Item::Table(servers)) = doc.get_mut("mcp_servers") {
            if servers.remove("codegraph").is_some() { changed = true; }
        }
        if changed {
            std::fs::write(config.as_std_path(), doc.to_string())?;
            Ok(InstallReport::Updated(vec![config]))
        } else {
            Ok(InstallReport::Unchanged)
        }
    }
}
