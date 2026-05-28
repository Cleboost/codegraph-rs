use crate::{
    targets::jsonutil, AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD,
};
use anyhow::Result;
use camino::Utf8PathBuf;
use serde_json::{json, Value};

pub struct CursorTarget;

impl CursorTarget {
    fn mcp_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::home_dir()?;
            Utf8PathBuf::from_path_buf(home.join(".cursor").join("mcp_config.json")).ok()
        } else {
            opts.project_root
                .as_ref()
                .map(|r| r.join(".cursor").join("mcp_config.json"))
        }
    }
    fn rule_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        opts.project_root
            .as_ref()
            .map(|r| r.join(".cursor").join("rules").join("codegraph.mdc"))
    }
}

impl AgentTarget for CursorTarget {
    fn id(&self) -> &'static str {
        "cursor"
    }
    fn label(&self) -> &'static str {
        "Cursor"
    }

    fn detect(&self, opts: &InstallOpts) -> DetectStatus {
        let Some(home) = opts.home_dir() else {
            return DetectStatus::NotFound;
        };
        if !home.join(".cursor").exists() {
            return DetectStatus::NotFound;
        }
        let Some(p) = self.mcp_path(opts) else {
            return DetectStatus::Found;
        };
        if !p.exists() {
            return DetectStatus::Found;
        }
        let Ok(v) = jsonutil::read_or_default(&p) else {
            return DetectStatus::Found;
        };
        if v.pointer("/mcpServers/codegraph").is_some() {
            DetectStatus::AlreadyConfigured
        } else {
            DetectStatus::Found
        }
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let mcp = self
            .mcp_path(opts)
            .ok_or_else(|| anyhow::anyhow!("no mcp path"))?;
        let mut v = jsonutil::read_or_default(&mcp)?;

        // Cursor MCP working-dir quirk: inject --path explicitly.
        let path_arg = match (&opts.project_root, opts.global) {
            (Some(root), false) => root.to_string(),
            _ => "${workspaceFolder}".to_string(),
        };
        let entry = json!({
            "command": opts.binary_path.as_str(),
            "args": ["serve", "--mcp", "--path", path_arg],
        });

        let mut changed = false;
        {
            let obj = v
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("mcp_config.json not an object"))?;
            let servers = obj
                .entry("mcpServers")
                .or_insert_with(|| Value::Object(Default::default()));
            let servers = servers
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("mcpServers not an object"))?;
            if servers.get("codegraph") != Some(&entry) {
                servers.insert("codegraph".into(), entry);
                changed = true;
            }
        }

        let mut written = Vec::new();
        if changed {
            jsonutil::write_pretty(&mcp, &v)?;
            written.push(mcp);
        }
        if let Some(rule) = self.rule_path(opts) {
            let want = format!(
                "---\ndescription: CodeGraph usage\nalwaysApply: true\n---\n\n{}",
                INSTRUCTIONS_MD
            );
            let existing = std::fs::read_to_string(rule.as_std_path()).ok();
            if existing.as_deref() != Some(want.as_str()) {
                if let Some(parent) = rule.parent() {
                    std::fs::create_dir_all(parent.as_std_path())?;
                }
                std::fs::write(rule.as_std_path(), &want)?;
                written.push(rule);
            }
        }
        if written.is_empty() {
            Ok(InstallReport::Unchanged)
        } else {
            Ok(InstallReport::Installed(written))
        }
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let mut removed = Vec::new();
        if let Some(mcp) = self.mcp_path(opts) {
            if mcp.exists() {
                let mut v = jsonutil::read_or_default(&mcp)?;
                let mut changed = false;
                if let Some(servers) = v.pointer_mut("/mcpServers").and_then(|s| s.as_object_mut())
                {
                    if servers.remove("codegraph").is_some() {
                        changed = true;
                    }
                }
                if changed {
                    jsonutil::write_pretty(&mcp, &v)?;
                    removed.push(mcp);
                }
            }
        }
        if removed.is_empty() {
            Ok(InstallReport::Unchanged)
        } else {
            Ok(InstallReport::Updated(removed))
        }
    }
}
