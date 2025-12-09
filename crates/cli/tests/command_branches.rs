use binary_slicer::commands::{
    add_binary_command, init_project_command, init_slice_command, project_info_command,
    show_ritual_run_command,
};
use tempfile::tempdir;

/// Exercise project_info human/json branches with populated binaries/slices.
#[test]
fn project_info_reports_binaries_and_slices() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("InfoProj".into())).unwrap();

    // Register a binary and a slice.
    let bin_path = temp.path().join("binA.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(
        &root,
        bin_path.to_str().unwrap(),
        Some("BinA".into()),
        Some("x86_64".into()),
        None,
        false,
    )
    .unwrap();
    init_slice_command(&root, "SliceA", Some("Test slice".into())).unwrap();

    // Human mode should print without errors.
    project_info_command(&root, false).unwrap();
    // JSON mode should include the entries.
    project_info_command(&root, true).unwrap();
}

/// Disk-only run directories should still allow show_ritual_run to report paths.
#[test]
fn show_ritual_run_with_spec_and_report_only() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("DiskOnlyRun".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let run_root = layout.binary_output_root("BinY").join("RunY");
    std::fs::create_dir_all(&run_root).unwrap();
    std::fs::write(run_root.join("spec.yaml"), "name: RunY\nbinary: BinY\nroots: [entry]\n")
        .unwrap();
    std::fs::write(run_root.join("report.json"), "{}").unwrap();
    show_ritual_run_command(&root, "BinY", "RunY", false).unwrap();
    show_ritual_run_command(&root, "BinY", "RunY", true).unwrap();
}
