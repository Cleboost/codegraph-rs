use camino::Utf8PathBuf;
use codegraph_installer::{project_registry as registry, DetectStatus, InstallOpts, InstallReport};

fn opts(root: &Utf8PathBuf) -> InstallOpts {
    InstallOpts {
        project_root: Some(root.clone()),
        global: false,
        binary_path: Utf8PathBuf::from("/usr/local/bin/codegraph"),
    }
}

#[test]
fn install_idempotent() {
    let d = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(d.path().to_path_buf()).unwrap();
    let o = opts(&root);
    for target in registry() {
        assert_eq!(target.detect(&o), DetectStatus::NotFound);
        let r1 = target.install(&o).unwrap();
        assert!(
            matches!(r1, InstallReport::Installed(_)),
            "{} first install: {:?}",
            target.id(),
            r1
        );
        assert_eq!(target.detect(&o), DetectStatus::AlreadyConfigured);
        let r2 = target.install(&o).unwrap();
        assert!(
            matches!(r2, InstallReport::Unchanged),
            "{} re-install: {:?}",
            target.id(),
            r2
        );
    }
}

#[test]
fn uninstall_removes_mcp_entry() {
    let d = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(d.path().to_path_buf()).unwrap();
    let o = opts(&root);
    for target in registry() {
        target.install(&o).unwrap();
        let r = target.uninstall(&o).unwrap();
        assert!(
            matches!(r, InstallReport::Updated(_)),
            "{} uninstall: {:?}",
            target.id(),
            r
        );
        assert_eq!(
            target.detect(&o),
            DetectStatus::Found,
            "{} should remain installed-but-not-configured",
            target.id()
        );
    }
}

#[test]
fn sibling_keys_preserved() {
    let d = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(d.path().to_path_buf()).unwrap();
    let claude_settings = root.join(".claude").join("settings.local.json");
    std::fs::create_dir_all(claude_settings.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(
        claude_settings.as_std_path(),
        r#"{"mcpServers":{"other":{"command":"foo"}},"theme":"dark"}"#,
    )
    .unwrap();
    let o = opts(&root);
    let claude = registry().into_iter().find(|t| t.id() == "claude").unwrap();
    claude.install(&o).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude_settings.as_std_path()).unwrap())
            .unwrap();
    assert!(
        v.pointer("/mcpServers/other").is_some(),
        "sibling MCP entry must survive"
    );
    assert_eq!(
        v.pointer("/theme").and_then(|v| v.as_str()),
        Some("dark"),
        "sibling field must survive"
    );
    assert!(v.pointer("/mcpServers/codegraph").is_some());
}
