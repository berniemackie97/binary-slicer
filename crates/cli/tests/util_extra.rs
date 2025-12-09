use binary_slicer::commands::open_project_db;
use binary_slicer::commands::util::{
    collect_ritual_runs_on_disk, collect_ritual_specs, load_runs_from_db_and_disk, print_dir_status,
};
use tempfile::tempdir;

#[test]
fn print_dir_status_handles_missing_and_existing_dirs() {
    let temp = tempdir().unwrap();
    let missing = temp.path().join("missing_dir");
    // Should not panic when directory is missing.
    print_dir_status("Missing", &missing);
    // And for existing dir.
    print_dir_status("Existing", temp.path());
}

#[test]
fn collect_ritual_runs_on_disk_merges_metadata() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(layout.outputs_binaries_dir.join("Bin1").join("Run1")).unwrap();
    let run_root = layout.outputs_binaries_dir.join("Bin1").join("Run1");
    let metadata = serde_json::json!({
        "ritual": "DemoRun",
        "binary": "Bin1",
        "started_at": "2025-01-01T00:00:00Z",
        "finished_at": "2025-01-01T00:00:01Z",
        "status": "succeeded",
        "spec_hash": "abc123",
        "binary_hash": "binhash",
        "backend": "capstone",
        "backend_version": "1.0"
    });
    std::fs::write(
        run_root.join("run_metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();
    let runs = collect_ritual_runs_on_disk(&layout, None).unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.binary, "Bin1");
    assert_eq!(run.name, "Run1");
    assert_eq!(run.status.as_deref(), Some("succeeded"));
    assert_eq!(run.backend.as_deref(), Some("capstone"));
}

#[test]
fn collect_ritual_specs_reads_yaml_and_json_formats() {
    let temp = tempdir().unwrap();
    let dir = temp.path();
    // YAML spec
    std::fs::write(dir.join("a.yaml"), "name: SpecA\nbinary: BinA\n").unwrap();
    // JSON spec
    std::fs::write(dir.join("b.json"), r#"{ "name": "SpecB", "binary": "BinB" }"#).unwrap();
    // Unsupported extension should be ignored.
    std::fs::write(dir.join("c.txt"), "ignore").unwrap();

    let specs = collect_ritual_specs(dir).unwrap();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "SpecA");
    assert_eq!(specs[1].name, "SpecB");
}

#[test]
fn load_runs_from_db_and_disk_backfills_disk_only_runs() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config = ritual_core::db::ProjectConfig::new("Proj", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();
    // Seed DB with no runs.
    let (_cfg, _db_path, _db) = open_project_db(&layout).unwrap();
    // Create a disk-only run.
    let run_dir = layout.outputs_binaries_dir.join("BinD").join("RunD");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(run_dir.join("spec.yaml"), "name: RunD\nbinary: BinD\n").unwrap();

    let runs = load_runs_from_db_and_disk(&layout, None).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].binary, "BinD");
    assert_eq!(runs[0].name, "RunD");
}
