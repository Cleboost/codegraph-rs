//! Antigravity CLI — Google's Go-based terminal agent (successor to Gemini CLI).
//! MCP config lives in a dedicated `mcp_config.json`, not inline in settings.
//! Global:    ~/.gemini/antigravity-cli/mcp_config.json
//! Workspace: .agents/mcp_config.json

use crate::{
    targets::jsonutil, AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD,
};
use anyhow::Result;
use camino::Utf8PathBuf;
use serde_json::{json, Value};

pub struct AntigravityTarget;

impl AntigravityTarget {
    fn mcp_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::home_dir()?;
            Utf8PathBuf::from_path_buf(
                home.join(".gemini")
                    .join("antigravity-cli")
                    .join("mcp_config.json"),
            )
            .ok()
        } else {
            opts.project_root
                .as_ref()
                .map(|r| r.join(".agents").join("mcp_config.json"))
        }
    }

    fn instructions_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::home_dir()?;
            Utf8PathBuf::from_path_buf(
                home.join(".gemini").join("antigravity-cli").join("AGENTS.md"),
            )
            .ok()
        } else {
            opts.project_root
                .as_ref()
                .map(|r| r.join(".agents").join("AGENTS.md"))
        }
    }
}

impl AgentTarget for AntigravityTarget {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn label(&self) -> &'static str {
        "Antigravity CLI"
    }

    fn detect(&self, opts: &InstallOpts) -> DetectStatus {
        // Agent presence: ~/.gemini/antigravity-cli/ must exist.
        let Some(home) = opts.home_dir() else {
            return DetectStatus::NotFound;
        };
        if !home.join(".gemini").join("antigravity-cli").exists() {
            return DetectStatus::NotFound;
        }
        // Check if codegraph is already configured in the target path.
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
            .ok_or_else(|| anyhow::anyhow!("no antigravity path"))?;
        let mut v = jsonutil::read_or_default(&mcp)?;

        let mut args = vec![Value::String("serve".into()), Value::String("--mcp".into())];
        if let Some(root) = &opts.project_root {
            args.push(Value::String("--path".into()));
            args.push(Value::String(root.to_string()));
        }
        let entry = json!({ "command": opts.binary_path.as_str(), "args": args });

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

        if let Some(md) = self.instructions_path(opts) {
            let existing = std::fs::read_to_string(md.as_std_path()).ok();
            if existing.as_deref() != Some(INSTRUCTIONS_MD) {
                if let Some(parent) = md.parent() {
                    std::fs::create_dir_all(parent.as_std_path())?;
                }
                std::fs::write(md.as_std_path(), INSTRUCTIONS_MD)?;
                written.push(md);
            }
        }

        if written.is_empty() {
            Ok(InstallReport::Unchanged)
        } else {
            Ok(InstallReport::Installed(written))
        }
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let Some(mcp) = self.mcp_path(opts) else {
            return Ok(InstallReport::Unchanged);
        };
        if !mcp.exists() {
            return Ok(InstallReport::Unchanged);
        }
        let mut v = jsonutil::read_or_default(&mcp)?;
        let mut changed = false;
        if let Some(servers) = v.pointer_mut("/mcpServers").and_then(|s| s.as_object_mut()) {
            if servers.remove("codegraph").is_some() {
                changed = true;
            }
        }
        if changed {
            jsonutil::write_pretty(&mcp, &v)?;
            Ok(InstallReport::Updated(vec![mcp]))
        } else {
            Ok(InstallReport::Unchanged)
        }
    }
}
