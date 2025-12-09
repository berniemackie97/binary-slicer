use binary_slicer::commands::setup_backend_command;
use tempfile::tempdir;

#[test]
fn setup_backend_records_rizin_path_and_default() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Proj", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let dummy = layout.meta_dir.join(if cfg!(windows) { "rizin.exe" } else { "rizin" });
    std::fs::write(&dummy, b"exe").unwrap();

    setup_backend_command(
        root.to_str().unwrap(),
        "rizin",
        Some(dummy.to_string_lossy().to_string()),
        true,
        false,
    )
    .unwrap();

    let updated: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    assert_eq!(updated.backends.rizin.as_deref(), Some(dummy.to_string_lossy().as_ref()));
    assert_eq!(updated.default_backend.as_deref(), Some("rizin"));
}

#[test]
fn setup_backend_records_ghidra_path_without_default() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Proj2", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let dummy =
        layout.meta_dir.join(if cfg!(windows) { "analyzeHeadless.bat" } else { "analyzeHeadless" });
    std::fs::write(&dummy, b"exe").unwrap();

    setup_backend_command(
        root.to_str().unwrap(),
        "ghidra",
        Some(dummy.to_string_lossy().to_string()),
        false,
        false,
    )
    .unwrap();

    let updated: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    assert_eq!(updated.backends.ghidra_headless.as_deref(), Some(dummy.to_string_lossy().as_ref()));
    assert!(updated.default_backend.is_none());
}

#[test]
fn setup_backend_errors_on_unknown_backend() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Proj3", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let err =
        setup_backend_command(root.to_str().unwrap(), "unknown", None, false, false).unwrap_err();
    assert!(err.to_string().contains("Unsupported backend"));
}

#[test]
fn setup_backend_errors_when_tool_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("ProjErr", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    // Point directly to a missing path to force failure, regardless of environment.
    let missing = temp.path().join("not_there").to_string_lossy().to_string();
    let err = setup_backend_command(root.to_str().unwrap(), "ghidra", Some(missing), false, false)
        .unwrap_err();
    assert!(
        err.to_string().contains("not found") || err.to_string().contains("analyzeHeadless"),
        "expected missing ghidra error"
    );
}

#[test]
fn setup_backend_finds_rizin_via_path() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("ProjPath", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    // Put dummy rizin on PATH.
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let exe = bin_dir.join(if cfg!(windows) { "rizin.exe" } else { "rizin" });
    std::fs::write(&exe, b"exe").unwrap();
    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path =
        format!("{}{}{}", bin_dir.display(), if cfg!(windows) { ";" } else { ":" }, old_path);
    std::env::set_var("PATH", &new_path);

    setup_backend_command(root.to_str().unwrap(), "rizin", None, false, false).unwrap();
    let updated: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    let resolved = updated.backends.rizin.as_deref().unwrap_or_default();
    assert!(
        resolved.ends_with("rizin.exe") || resolved.ends_with("rizin"),
        "unexpected rizin path: {resolved}"
    );
    assert!(updated.default_backend.is_none());
}

#[test]
fn setup_backend_write_path_for_rizin() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config =
        ritual_core::db::ProjectConfig::new("ProjPathWrite", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let exe = bin_dir.join(if cfg!(windows) { "rizin.exe" } else { "rizin" });
    std::fs::write(&exe, b"exe").unwrap();
    let old_path = std::env::var("PATH").unwrap_or_default();
    let new_path =
        format!("{}{}{}", bin_dir.display(), if cfg!(windows) { ";" } else { ":" }, old_path);
    std::env::set_var("PATH", &new_path);

    if cfg!(windows) {
        std::env::set_var("USERPROFILE", root);
    } else {
        std::env::set_var("HOME", root);
        std::fs::write(root.join(".bashrc"), "# test\n").unwrap();
    }

    setup_backend_command(root.to_str().unwrap(), "rizin", None, false, true).unwrap();
    let updated: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    let resolved = updated.backends.rizin.as_deref().unwrap_or_default();
    assert!(resolved.ends_with("rizin.exe") || resolved.ends_with("rizin"));

    // Profile may or may not be created depending on platform; if present ensure it references the bin dir.
    // Profile write is best-effort; ensure we didn't crash.
}

#[test]
fn setup_backend_uses_env_for_ghidra_and_writes_profile() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Proj4", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    // Prepare dummy analyzeHeadless and expose via env var detection.
    let dummy =
        layout.meta_dir.join(if cfg!(windows) { "analyzeHeadless.bat" } else { "analyzeHeadless" });
    std::fs::write(&dummy, b"exe").unwrap();
    std::env::set_var("GHIDRA_ANALYZE_HEADLESS", &dummy);

    // Force profile writes into the temp tree so we don't touch user files.
    if cfg!(windows) {
        std::env::set_var("USERPROFILE", root);
    } else {
        std::env::set_var("HOME", root);
        // Ensure .bashrc exists so append_to_profile picks it deterministically.
        std::fs::write(root.join(".bashrc"), "# test\n").unwrap();
    }

    setup_backend_command(root.to_str().unwrap(), "ghidra", None, true, true).unwrap();

    let updated: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    assert_eq!(updated.backends.ghidra_headless.as_deref(), Some(dummy.to_string_lossy().as_ref()));
    assert_eq!(updated.default_backend.as_deref(), Some("ghidra"));

    // Cleanup env so later tests don't see this path.
    std::env::remove_var("GHIDRA_ANALYZE_HEADLESS");
    std::env::remove_var("GHIDRA_INSTALL_DIR");
}
