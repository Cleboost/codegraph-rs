use crate::{targets::jsonutil, AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD};
use anyhow::Result;
use camino::Utf8PathBuf;
use serde_json::{json, Value};

pub struct ClaudeTarget;

impl ClaudeTarget {
    fn settings_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::home_dir()?;
            Utf8PathBuf::from_path_buf(home.join(".claude").join("settings.json")).ok()
        } else {
            let root = opts.project_root.as_ref()?;
            Some(root.join(".claude").join("settings.local.json"))
        }
    }
    fn instructions_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::home_dir()?;
            Utf8PathBuf::from_path_buf(home.join(".claude").join("CLAUDE.md")).ok()
        } else {
            opts.project_root.as_ref().map(|r| r.join("CLAUDE.md"))
        }
    }
}

impl AgentTarget for ClaudeTarget {
    fn id(&self) -> &'static str { "claude" }
    fn label(&self) -> &'static str { "Claude Code" }

    fn detect(&self, opts: &InstallOpts) -> DetectStatus {
        let Some(p) = self.settings_path(opts) else { return DetectStatus::NotFound };
        if !p.exists() { return DetectStatus::NotFound; }
        let Ok(v) = jsonutil::read_or_default(&p) else { return DetectStatus::Found };
        if v.pointer("/mcpServers/codegraph").is_some() {
            DetectStatus::AlreadyConfigured
        } else {
            DetectStatus::Found
        }
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let settings = self.settings_path(opts).ok_or_else(|| anyhow::anyhow!("no settings path"))?;
        let mut v = jsonutil::read_or_default(&settings)?;

        let mcp_entry = json!({
            "command": opts.binary_path.as_str(),
            "args": serve_args(opts),
        });
        let mut changed = false;
        {
            let obj = v.as_object_mut().ok_or_else(|| anyhow::anyhow!("settings.json not an object"))?;
            let servers = obj.entry("mcpServers").or_insert_with(|| Value::Object(Default::default()));
            let servers = servers.as_object_mut().ok_or_else(|| anyhow::anyhow!("mcpServers not an object"))?;
            if servers.get("codegraph") != Some(&mcp_entry) {
                servers.insert("codegraph".into(), mcp_entry);
                changed = true;
            }
        }

        let mut written = Vec::new();
        if changed {
            jsonutil::write_pretty(&settings, &v)?;
            written.push(settings);
        }

        if let Some(md) = self.instructions_path(opts) {
            let want = INSTRUCTIONS_MD;
            let existing = std::fs::read_to_string(md.as_std_path()).ok();
            if existing.as_deref() != Some(want) {
                if let Some(parent) = md.parent() { std::fs::create_dir_all(parent.as_std_path())?; }
                std::fs::write(md.as_std_path(), want)?;
                written.push(md);
            }
        }

        if written.is_empty() { Ok(InstallReport::Unchanged) } else { Ok(InstallReport::Installed(written)) }
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let mut removed = Vec::new();
        if let Some(settings) = self.settings_path(opts) {
            if settings.exists() {
                let mut v = jsonutil::read_or_default(&settings)?;
                let mut changed = false;
                if let Some(servers) = v.pointer_mut("/mcpServers").and_then(|s| s.as_object_mut()) {
                    if servers.remove("codegraph").is_some() { changed = true; }
                }
                if changed {
                    jsonutil::write_pretty(&settings, &v)?;
                    removed.push(settings);
                }
            }
        }
        if removed.is_empty() { Ok(InstallReport::Unchanged) } else { Ok(InstallReport::Updated(removed)) }
    }
}

fn serve_args(opts: &InstallOpts) -> Value {
    let mut args = vec![Value::String("serve".into()), Value::String("--mcp".into())];
    if let Some(root) = &opts.project_root {
        args.push(Value::String("--path".into()));
        args.push(Value::String(root.to_string()));
    }
    Value::Array(args)
}
