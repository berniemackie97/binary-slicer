use binary_slicer::commands::{
    clean_outputs_command, emit_slice_docs_command, emit_slice_reports_command,
    init_project_command, list_ritual_runs_command, list_slices_command, project_info_command,
    setup_backend_command, show_ritual_run_command,
};
use tempfile::tempdir;

#[test]
fn list_slices_errors_when_config_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    let err = list_slices_command(&root, false).unwrap_err();
    assert!(err.to_string().contains("Failed to read project config"), "unexpected error: {err}");
}

#[test]
fn emit_slice_reports_errors_when_db_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("ErrSlices".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let mut cfg: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    cfg.db.path = ".ritual/project.json/bad.db".into();
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&cfg).unwrap())
        .unwrap();
    let err = emit_slice_reports_command(&root).unwrap_err();
    assert!(err.to_string().contains("Failed to open project database"), "unexpected error: {err}");
}

#[test]
fn project_info_errors_when_config_corrupt() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("CorruptProj".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    std::fs::write(&layout.project_config_path, "not-json").unwrap();
    let err = project_info_command(&root, true).unwrap_err();
    assert!(err.to_string().contains("Failed to parse project config JSON"));
}

#[test]
fn clean_outputs_requires_binary_when_ritual_specified() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    let err = clean_outputs_command(&root, None, Some("Run"), false, true).unwrap_err();
    assert!(err.to_string().contains("--ritual requires --binary"));
}

#[test]
fn clean_outputs_requires_binary_or_all_flag() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    let err = clean_outputs_command(&root, None, None, false, true).unwrap_err();
    assert!(err.to_string().contains("Specify --binary or use --all"));
}

#[test]
fn show_ritual_run_handles_missing_metadata_but_existing_dir() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("NoMetaProj".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let run_root = layout.binary_output_root("BinX").join("RunX");
    std::fs::create_dir_all(&run_root).unwrap(); // dir exists but no metadata/spec/report
    show_ritual_run_command(&root, "BinX", "RunX", false).unwrap();
}

#[test]
fn setup_backend_requires_path_or_env_for_ghidra() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Cfg", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let err =
        setup_backend_command(root.to_str().unwrap(), "ghidra", None, false, false).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("ghidra"),
        "unexpected ghidra path error: {err}"
    );
}

#[test]
fn list_ritual_runs_filter_no_matches() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("NoRunsProj".into())).unwrap();
    // Should print none even with a filter that does not match.
    list_ritual_runs_command(&root, Some("MissingBin"), false).unwrap();
}

#[test]
fn emit_slice_docs_errors_when_db_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("ErrDocs".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let mut cfg: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    cfg.db.path = ".ritual/project.json/bad_docs.db".into();
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&cfg).unwrap())
        .unwrap();
    let err = emit_slice_docs_command(&root).unwrap_err();
    assert!(err.to_string().contains("Failed to open project database"), "unexpected error: {err}");
}

#[test]
fn clean_outputs_nothing_to_remove_reports_success() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("CleanNone".into())).unwrap();
    // No outputs exist; should print "Nothing to remove" and succeed.
    clean_outputs_command(&root, Some("NonexistentBin"), None, false, true).unwrap();
}

#[test]
fn project_info_errors_when_db_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("NoDB".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    // Corrupt config to point at a bad DB path so open fails.
    let mut cfg: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&layout.project_config_path).unwrap())
            .unwrap();
    cfg.db.path = ".ritual/missing_dir/db.sqlite".into();
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&cfg).unwrap())
        .unwrap();
    let err = project_info_command(&root, true).unwrap_err();
    assert!(
        err.to_string().contains("Failed to open project database"),
        "unexpected project info error: {err}"
    );
}

#[test]
fn init_project_errors_when_meta_dir_is_file() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    // Precreate a file where the .ritual directory should live to force create_dir_all to fail.
    let meta_file = root.join(".ritual");
    std::fs::write(&meta_file, b"block").unwrap();

    let err = init_project_command(root.to_string_lossy().as_ref(), Some("BlockedProj".into()))
        .unwrap_err();
    assert!(err.to_string().contains("Failed to create meta dir"), "unexpected error: {err}");
}

#[test]
fn add_binary_errors_when_config_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    // Write a dummy binary so we progress far enough to attempt config load.
    let bin_path = temp.path().join("bin_missing_cfg.so");
    std::fs::write(&bin_path, b"payload").unwrap();

    let err = binary_slicer::commands::add_binary_command(
        &root,
        bin_path.to_str().unwrap(),
        None,
        None,
        None,
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("Failed to read project config"), "unexpected error: {err}");
}

#[test]
fn list_binaries_errors_when_config_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    let err = binary_slicer::commands::list_binaries_command(&root, false).unwrap_err();
    assert!(err.to_string().contains("Failed to read project config"), "unexpected error: {err}");
}

#[test]
fn list_ritual_runs_errors_when_config_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    let err = list_ritual_runs_command(&root, None, false).unwrap_err();
    assert!(err.to_string().contains("Failed to read project config"), "unexpected error: {err}");
}

#[test]
fn sha256_file_errors_with_context() {
    let temp = tempdir().unwrap();
    let missing = temp.path().join("missing.bin");
    let err = binary_slicer::sha256_file(&missing).unwrap_err();
    assert!(err.to_string().contains("Failed to open binary for hashing"));
}

#[test]
fn show_ritual_run_uses_disk_when_spec_and_report_exist() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("DiskOnlyShow".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let run_root = layout.binary_output_root("BinDisk").join("RunDisk");
    std::fs::create_dir_all(&run_root).unwrap();
    std::fs::write(run_root.join("spec.yaml"), "name: RunDisk\nbinary: BinDisk\nroots: [entry]\n")
        .unwrap();
    std::fs::write(run_root.join("report.json"), "{}").unwrap();
    // Should not error even without DB metadata.
    show_ritual_run_command(&root, "BinDisk", "RunDisk", true).unwrap();
}

#[test]
fn clean_outputs_all_mode_with_missing_outputs_dir() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("CleanAllNone".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    std::fs::remove_dir_all(&layout.outputs_dir).unwrap();
    clean_outputs_command(&root, None, None, true, true).unwrap();
}
