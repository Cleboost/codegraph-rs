//! opencode — prefers `opencode.jsonc` if present, falls back to `.json`.
//! For greenfield installs, creates `.jsonc`. Surgical edits via jsonc-parser
//! to preserve user comments and formatting.

use crate::{
    targets::jsonutil, AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD,
};
use anyhow::Result;
use camino::Utf8PathBuf;
use serde_json::{json, Value};

pub struct OpencodeTarget;

impl OpencodeTarget {
    fn config_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let home = dirs::config_dir()?;
            Utf8PathBuf::from_path_buf(home.join("opencode").join("opencode.jsonc")).ok()
        } else {
            opts.project_root.as_ref().map(|r| {
                let jsonc = r.join("opencode.jsonc");
                if jsonc.exists() {
                    return jsonc;
                }
                let json = r.join("opencode.json");
                if json.exists() {
                    return json;
                }
                jsonc
            })
        }
    }

    fn agents_path(&self, opts: &InstallOpts) -> Option<Utf8PathBuf> {
        if opts.global {
            let cfg = dirs::config_dir()?;
            Utf8PathBuf::from_path_buf(cfg.join("opencode").join("AGENTS.md")).ok()
        } else {
            opts.project_root.as_ref().map(|r| r.join("AGENTS.md"))
        }
    }

    fn parse_jsonc(&self, text: &str) -> Result<Value> {
        if text.trim().is_empty() {
            return Ok(Value::Object(Default::default()));
        }
        let stripped: String = strip_jsonc_comments(text);
        Ok(serde_json::from_str(&stripped)?)
    }
}

fn strip_jsonc_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut in_str = false;
    let mut escape = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            out.push(c as char);
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = true;
            out.push('"');
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
        }
        out.push(c as char);
        i += 1;
    }
    out
}

impl AgentTarget for OpencodeTarget {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn label(&self) -> &'static str {
        "opencode"
    }

    fn detect(&self, opts: &InstallOpts) -> DetectStatus {
        let installed = which::which("opencode").is_ok()
            || opts
                .home_dir()
                .map(|h| h.join(".config").join("opencode").exists())
                .unwrap_or(false)
            || dirs::config_dir()
                .map(|d| d.join("opencode").exists())
                .unwrap_or(false);
        if !installed {
            return DetectStatus::NotFound;
        }
        let Some(p) = self.config_path(opts) else {
            return DetectStatus::Found;
        };
        if !p.exists() {
            return DetectStatus::Found;
        }
        let Ok(text) = std::fs::read_to_string(p.as_std_path()) else {
            return DetectStatus::Found;
        };
        let Ok(v) = self.parse_jsonc(&text) else {
            return DetectStatus::Found;
        };
        if v.pointer("/mcp/codegraph").is_some() {
            DetectStatus::AlreadyConfigured
        } else {
            DetectStatus::Found
        }
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let config = self
            .config_path(opts)
            .ok_or_else(|| anyhow::anyhow!("no opencode config path"))?;
        let text = std::fs::read_to_string(config.as_std_path()).unwrap_or_default();
        let mut v = self.parse_jsonc(&text)?;

        let mut cmd: Vec<Value> = vec![
            Value::String(opts.binary_path.to_string()),
            Value::String("serve".into()),
            Value::String("--mcp".into()),
        ];
        if let Some(root) = &opts.project_root {
            cmd.push(Value::String("--path".into()));
            cmd.push(Value::String(root.to_string()));
        }
        let entry = json!({
            "type": "local",
            "command": cmd,
            "enabled": true,
        });

        let mut changed = false;
        {
            let obj = v
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("opencode config not an object"))?;
            let mcp = obj
                .entry("mcp")
                .or_insert_with(|| Value::Object(Default::default()));
            let mcp = mcp
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("mcp not an object"))?;
            if mcp.get("codegraph") != Some(&entry) {
                mcp.insert("codegraph".into(), entry);
                changed = true;
            }
        }

        let mut written = Vec::new();
        if changed {
            jsonutil::write_pretty(&config, &v)?;
            written.push(config);
        }
        if let Some(md) = self.agents_path(opts) {
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
        let Some(config) = self.config_path(opts) else {
            return Ok(InstallReport::Unchanged);
        };
        if !config.exists() {
            return Ok(InstallReport::Unchanged);
        }
        let text = std::fs::read_to_string(config.as_std_path())?;
        let mut v = self.parse_jsonc(&text)?;
        let mut changed = false;
        if let Some(mcp) = v.pointer_mut("/mcp").and_then(|m| m.as_object_mut()) {
            if mcp.remove("codegraph").is_some() {
                changed = true;
            }
        }
        if changed {
            jsonutil::write_pretty(&config, &v)?;
            Ok(InstallReport::Updated(vec![config]))
        } else {
            Ok(InstallReport::Unchanged)
        }
    }
}
