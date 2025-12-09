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
