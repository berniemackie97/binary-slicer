use assert_cmd::cargo::cargo_bin_cmd;
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
    init_slice_command(&root, "SliceA", Some("Test slice".into()), None).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);
    let db = ritual_core::db::ProjectDb::open(&layout.db_path).unwrap();
    let run = ritual_core::db::RitualRunRecord {
        binary: "BinA".into(),
        ritual: "SliceA".into(),
        spec_hash: "sh".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: ritual_core::db::RitualRunStatus::Succeeded,
        started_at: "t0".into(),
        finished_at: "t1".into(),
    };
    db.insert_ritual_run(&run).unwrap();

    // Human mode via CLI should print backend info for the run.
    let human_out = cargo_bin_cmd!("binary-slicer")
        .arg("project-info")
        .arg("--root")
        .arg(&root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8_lossy(&human_out);
    assert!(human.contains("backend: rizin"));
    assert!(human.contains("/usr/bin/rizin"));
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
