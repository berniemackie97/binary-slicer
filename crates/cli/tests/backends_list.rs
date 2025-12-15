use assert_cmd::cargo::cargo_bin_cmd;
use binary_slicer::commands::list_backends_command;
use tempfile::tempdir;

#[test]
fn list_backends_reports_available_backends() {
    // Should succeed in both human and JSON modes.
    list_backends_command(false).unwrap();
    list_backends_command(true).unwrap();
}

#[test]
fn configured_backends_round_trip_in_project_info() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    // Initialize project, then set backends and ensure project-info JSON surfaces them.
    binary_slicer::commands::init_project_command(&root, Some("BackendsProj".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let mut cfg: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    cfg.backends.rizin = Some("/usr/bin/rizin".into());
    cfg.backend_versions.rizin = Some("rz-1.2".into());
    cfg.default_backend = Some("rizin".into());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&cfg).unwrap())
        .unwrap();

    // Should include backends and versions in JSON output.
    let output = cargo_bin_cmd!("binary-slicer")
        .arg("project-info")
        .arg("--root")
        .arg(&root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let body: serde_json::Value = serde_json::from_slice(&output).expect("project-info json");
    assert_eq!(body["backends"]["rizin"], "/usr/bin/rizin");
    assert_eq!(body["backend_versions"]["rizin"], "rz-1.2");
}

#[test]
fn configured_backend_paths_returns_configured_values() {
    let mut cfg = ritual_core::db::ProjectConfig::new("CfgPaths", ".ritual/project.db");
    cfg.backends.rizin = Some("/usr/bin/rizin".into());
    let paths = binary_slicer::commands::configured_backend_paths(&cfg);
    assert_eq!(paths.rizin.as_deref(), Some("/usr/bin/rizin"));
}
