use crate::{
    targets::jsonutil, AgentTarget, DetectStatus, InstallOpts, InstallReport, INSTRUCTIONS_MD,
};
use anyhow::Result;
use camino::Utf8PathBuf;
use serde_json::{json, Value};

pub struct AntigravityTarget;

impl AntigravityTarget {
    fn config_paths(&self, _opts: &InstallOpts) -> Vec<Utf8PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = dirs::home_dir() {
            if let Ok(p) = Utf8PathBuf::from_path_buf(home.join(".gemini").join("config").join("plugins").join("codegraph").join("mcp_config.json")) {
                paths.push(p);
            }
        }
        paths
    }
    
    fn instructions_paths(&self, _opts: &InstallOpts) -> Vec<Utf8PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = dirs::home_dir() {
            if let Ok(p) = Utf8PathBuf::from_path_buf(home.join(".gemini").join("config").join("plugins").join("codegraph").join("rules").join("codegraph.md")) {
                paths.push(p);
            }
        }
        paths
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
        let paths = self.config_paths(opts);
        if paths.is_empty() {
            return DetectStatus::NotFound;
        }

        let mut all_not_found = true;
        let mut any_configured = false;

        for p in &paths {
            if !p.exists() {
                if let Some(parent) = p.parent() {
                    if parent.exists() {
                        all_not_found = false;
                    }
                }
            } else {
                all_not_found = false;
                if let Ok(v) = jsonutil::read_or_default(p) {
                    if v.pointer("/mcpServers/codegraph").is_some() {
                        any_configured = true;
                    }
                }
            }
        }

        if any_configured {
            DetectStatus::AlreadyConfigured
        } else if all_not_found {
            DetectStatus::NotFound
        } else {
            DetectStatus::Found
        }
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport> {
        let paths = self.config_paths(opts);
        if paths.is_empty() {
            return Err(anyhow::anyhow!("no settings paths"));
        }

        let mut written = Vec::new();
        let mcp_entry = json!({
            "command": opts.binary_path.as_str(),
            "args": serve_args(opts),
        });

        for settings in &paths {
            if let Some(parent) = settings.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent.as_std_path())?;
                }
            }
                
            let mut v = jsonutil::read_or_default(settings)?;
            let mut changed = false;
            {
                if !v.is_object() {
                    v = Value::Object(Default::default());
                }
                let obj = v.as_object_mut().unwrap();
                let servers = obj
                    .entry("mcpServers")
                    .or_insert_with(|| Value::Object(Default::default()));
                let servers = servers
                    .as_object_mut()
                    .ok_or_else(|| anyhow::anyhow!("mcpServers not an object"))?;
                if servers.get("codegraph") != Some(&mcp_entry) {
                    servers.insert("codegraph".into(), mcp_entry.clone());
                    changed = true;
                }
            }

            if changed {
                jsonutil::write_pretty(settings, &v)?;
                written.push(settings.clone());
            }

            // Write plugin.json marker
            if let Some(parent) = settings.parent() {
                let plugin_json = parent.join("plugin.json");
                if !plugin_json.exists() {
                    let manifest = json!({ "name": "codegraph" });
                    let _ = jsonutil::write_pretty(&plugin_json, &manifest);
                    written.push(plugin_json);
                }
            }
        }

        for md in self.instructions_paths(opts) {
            let want = INSTRUCTIONS_MD;
            let existing = std::fs::read_to_string(md.as_std_path()).ok();
            if existing.as_deref() != Some(want) {
                if let Some(parent) = md.parent() {
                    std::fs::create_dir_all(parent.as_std_path())?;
                }
                std::fs::write(md.as_std_path(), want)?;
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
        let mut removed = Vec::new();
        for settings in self.config_paths(opts) {
            if settings.exists() {
                let mut v = jsonutil::read_or_default(&settings)?;
                let mut changed = false;
                if let Some(servers) = v.pointer_mut("/mcpServers").and_then(|s| s.as_object_mut())
                {
                    if servers.remove("codegraph").is_some() {
                        changed = true;
                    }
                }
                if changed {
                    jsonutil::write_pretty(&settings, &v)?;
                    removed.push(settings);
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

fn serve_args(opts: &InstallOpts) -> Value {
    let mut args = vec![Value::String("serve".into()), Value::String("--mcp".into())];
    if let Some(root) = &opts.project_root {
        args.push(Value::String("--path".into()));
        args.push(Value::String(root.to_string()));
    }
    Value::Array(args)
}
