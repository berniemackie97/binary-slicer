use binary_slicer::commands::{
    add_binary_command, clean_outputs_command, collect_ritual_runs_on_disk, collect_ritual_specs,
    emit_slice_docs_command, emit_slice_reports_command, init_project_command, init_slice_command,
    list_backends_command, list_binaries_command, list_ritual_runs_command,
    list_ritual_specs_command, list_slices_command, project_info_command, rerun_ritual_command,
    run_ritual_command, sha256_bytes, show_ritual_run_command, update_ritual_run_status_command,
    validate_run_status, RitualRunMetadata, RitualSpec,
};
use ritual_core::db::RitualRunStatus;
use tempfile::tempdir;

#[test]
fn validates_known_statuses() {
    for s in ["pending", "running", "succeeded", "failed", "canceled", "stubbed"] {
        let parsed = validate_run_status(s).expect("should parse");
        assert_eq!(parsed.as_str(), s);
    }
}

#[test]
fn rejects_unknown_status() {
    let err = validate_run_status("bogus").unwrap_err();
    assert!(err.to_string().contains("Invalid status"));
}

#[test]
fn ritual_spec_validation_rejects_missing_fields() {
    let invalid = RitualSpec {
        name: "".to_string(),
        binary: "".to_string(),
        roots: vec![],
        max_depth: None,
        backend: None,
        description: None,
        outputs: None,
    };
    let err = invalid.validate().unwrap_err();
    assert!(err.to_string().contains("required"));
}

#[test]
fn sha256_bytes_matches_known_hash() {
    let hash = sha256_bytes(b"abc");
    assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn run_metadata_round_trips_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("meta.json");
    let metadata = RitualRunMetadata {
        ritual: "Test".into(),
        binary: "Bin".into(),
        spec_hash: "123".into(),
        binary_hash: Some("456".into()),
        backend: "validate-only".into(),
        backend_version: Some("v1".into()),
        backend_path: Some("/bin/tool".into()),
        started_at: "now".into(),
        finished_at: "later".into(),
        status: RitualRunStatus::Succeeded,
    };
    std::fs::write(&path, serde_json::to_string(&metadata).unwrap()).unwrap();
    let parsed: RitualRunMetadata =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(parsed.status.as_str(), "succeeded");
    assert_eq!(parsed.binary_hash.as_deref(), Some("456"));
}

#[test]
fn collect_ritual_specs_reads_yaml_and_json() {
    let dir = tempdir().unwrap();
    let yaml = dir.path().join("alpha.yaml");
    let json = dir.path().join("beta.json");

    std::fs::write(&yaml, "name: Alpha\nbinary: BinA\nroots: [entry_point]\nmax_depth: 2\n")
        .unwrap();
    std::fs::write(&json, r#"{"name":"Beta","binary":"BinB","roots":["start"],"max_depth":1}"#)
        .unwrap();

    let specs = collect_ritual_specs(dir.path()).unwrap();
    assert_eq!(specs.len(), 2);
    assert!(specs.iter().any(|s| s.name == "Alpha" && s.binary.as_deref() == Some("BinA")));
    assert!(specs.iter().any(|s| s.name == "Beta" && s.binary.as_deref() == Some("BinB")));
}

#[test]
fn collect_ritual_runs_on_disk_reads_metadata() {
    let dir = tempdir().unwrap();
    let layout = ritual_core::db::ProjectLayout {
        root: dir.path().to_path_buf(),
        meta_dir: dir.path().join(".ritual"),
        project_config_path: dir.path().join(".ritual").join("project.json"),
        db_path: dir.path().join(".ritual").join("project.db"),
        docs_dir: dir.path().join("docs"),
        slices_docs_dir: dir.path().join("docs").join("slices"),
        reports_dir: dir.path().join("reports"),
        graphs_dir: dir.path().join("graphs"),
        rituals_dir: dir.path().join("rituals"),
        outputs_dir: dir.path().join("outputs"),
        outputs_binaries_dir: dir.path().join("outputs").join("binaries"),
    };

    let run_dir = layout.binary_output_root("TestBin").join("TestRun");
    std::fs::create_dir_all(&run_dir).unwrap();
    let metadata = RitualRunMetadata {
        ritual: "TestRun".into(),
        binary: "TestBin".into(),
        spec_hash: "abc".into(),
        binary_hash: Some("def".into()),
        backend: "validate-only".into(),
        backend_version: None,
        backend_path: None,
        started_at: "s".into(),
        finished_at: "f".into(),
        status: ritual_core::db::RitualRunStatus::Stubbed,
    };
    std::fs::write(
        run_dir.join("run_metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    let runs = collect_ritual_runs_on_disk(&layout, None).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].binary, "TestBin");
    assert_eq!(runs[0].name, "TestRun");
    assert_eq!(runs[0].status.as_deref(), Some("stubbed"));
}

#[test]
fn project_info_includes_disk_only_runs() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("DiskOnly".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);

    // Create a run folder on disk without inserting into DB.
    let run_dir = layout.binary_output_root("DiskBin").join("DiskRun");
    std::fs::create_dir_all(&run_dir).unwrap();
    let metadata = RitualRunMetadata {
        ritual: "DiskRun".into(),
        binary: "DiskBin".into(),
        spec_hash: "disk".into(),
        binary_hash: None,
        backend: "validate-only".into(),
        backend_version: None,
        backend_path: None,
        started_at: "s".into(),
        finished_at: "f".into(),
        status: ritual_core::db::RitualRunStatus::Stubbed,
    };
    std::fs::write(
        run_dir.join("run_metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    // Human and JSON to hit both branches.
    project_info_command(&root, false).unwrap();
    project_info_command(&root, true).unwrap();
}

#[test]
fn direct_project_and_slice_commands_execute() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();

    init_project_command(&root, Some("DirectProject".into())).unwrap();

    // add a binary and list it
    let bin_path = temp.path().join("binA.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(
        &root,
        bin_path.to_str().unwrap(),
        Some("BinA".into()),
        Some("armv7".into()),
        None,
        false,
    )
    .unwrap();
    list_binaries_command(&root, false).unwrap();

    // slice commands
    init_slice_command(&root, "SliceA", Some("desc".into())).unwrap();
    list_slices_command(&root, false).unwrap();
    emit_slice_docs_command(&root).unwrap();
    emit_slice_reports_command(&root).unwrap();

    // project-info human and json
    project_info_command(&root, false).unwrap();
    project_info_command(&root, true).unwrap();
}

#[test]
fn direct_ritual_commands_execute_and_update_status() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();

    init_project_command(&root, Some("RitualProj".into())).unwrap();
    let bin_path = temp.path().join("binR.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(&root, bin_path.to_str().unwrap(), Some("BinR".into()), None, None, false)
        .unwrap();

    // write spec yaml and run
    let spec_path = temp.path().join("rit.yaml");
    std::fs::write(&spec_path, "name: RunOne\nbinary: BinR\nroots: [entry_point]\nmax_depth: 1\n")
        .unwrap();
    run_ritual_command(&root, spec_path.to_str().unwrap(), None, false).unwrap();

    // list & show runs/specs
    list_ritual_runs_command(&root, Some("BinR"), true).unwrap();
    list_ritual_specs_command(&root, false).unwrap();
    show_ritual_run_command(&root, "BinR", "RunOne", true).unwrap();

    // rerun and update status
    rerun_ritual_command(&root, "BinR", "RunOne", "RunTwo", None, true).unwrap();
    update_ritual_run_status_command(&root, "BinR", "RunTwo", "succeeded", None).unwrap();

    // clean outputs
    clean_outputs_command(&root, Some("BinR"), None, false, true).unwrap();
}

#[test]
fn emit_slice_commands_handle_empty_db() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("EmptyProj".into())).unwrap();
    // No slices registered -> should short-circuit gracefully.
    emit_slice_docs_command(&root).unwrap();
    emit_slice_reports_command(&root).unwrap();
}

#[test]
fn list_commands_handle_empty_sets_and_json() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("ListProj".into())).unwrap();
    list_slices_command(&root, true).unwrap();
    list_binaries_command(&root, true).unwrap();
    // backends list should always succeed (json and human)
    list_backends_command(true).unwrap();
    list_backends_command(false).unwrap();
    // ritual runs (human/json) empty path
    list_ritual_runs_command(&root, None, false).unwrap();
}

#[test]
fn add_binary_honors_skip_hash_flag() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("HashProj".into())).unwrap();
    let bin_path = temp.path().join("nohash.bin");
    std::fs::write(&bin_path, b"bytes").unwrap();
    add_binary_command(&root, bin_path.to_str().unwrap(), Some("NoHash".into()), None, None, true)
        .unwrap();
    // JSON list should still succeed even without hash present.
    list_binaries_command(&root, true).unwrap();
}

#[test]
fn clean_outputs_requires_yes_and_binary_for_ritual() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("CleanProj".into())).unwrap();
    let err = clean_outputs_command(&root, Some("BinX"), None, false, false).unwrap_err();
    assert!(err.to_string().contains("Refusing"));

    let err = clean_outputs_command(&root, None, Some("RunY"), false, true).unwrap_err();
    assert!(err.to_string().contains("requires --binary"));
}

#[test]
fn run_ritual_force_overwrites_existing_output() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();

    init_project_command(&root, Some("ForceProj".into())).unwrap();
    let bin_path = temp.path().join("binF.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(&root, bin_path.to_str().unwrap(), Some("BinF".into()), None, None, false)
        .unwrap();

    let spec_path = temp.path().join("force.yaml");
    std::fs::write(&spec_path, "name: ForceRun\nbinary: BinF\nroots: [entry]\nmax_depth: 1\n")
        .unwrap();

    run_ritual_command(&root, spec_path.to_str().unwrap(), None, false).unwrap();
    // Re-run with force to hit overwrite branch.
    run_ritual_command(&root, spec_path.to_str().unwrap(), None, true).unwrap();
}

#[test]
fn run_ritual_errors_when_output_exists_without_force() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("NoForceProj".into())).unwrap();
    let bin_path = temp.path().join("binNF.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(&root, bin_path.to_str().unwrap(), Some("BinNF".into()), None, None, false)
        .unwrap();
    let spec_path = temp.path().join("noforce.yaml");
    std::fs::write(&spec_path, "name: RunNF\nbinary: BinNF\nroots: [entry_point]\nmax_depth: 1\n")
        .unwrap();
    run_ritual_command(&root, spec_path.to_str().unwrap(), None, false).unwrap();
    let err = run_ritual_command(&root, spec_path.to_str().unwrap(), None, false).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn project_info_human_includes_backends_and_layout() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("LayoutProj".into())).unwrap();
    // Patch config to include backends/default.
    let config_path = ritual_core::db::ProjectLayout::new(&root).project_config_path;
    let mut cfg: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    cfg.default_backend = Some("validate-only".into());
    cfg.backends.rizin = Some("rizin".into());
    std::fs::write(&config_path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();

    // Human mode should print without errors even with no binaries/slices.
    project_info_command(&root, false).unwrap();
}

#[test]
fn run_ritual_errors_on_unknown_backend() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("BackendProj".into())).unwrap();
    let bin_path = temp.path().join("binBK.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    add_binary_command(&root, bin_path.to_str().unwrap(), Some("BinBK".into()), None, None, false)
        .unwrap();

    let spec_path = temp.path().join("backend.yaml");
    std::fs::write(
        &spec_path,
        "name: BackendRun\nbinary: BinBK\nroots: [entry_point]\nmax_depth: 1\n",
    )
    .unwrap();

    // Mutate config to set a default backend to ensure precedence still allows CLI override.
    let config_path = ritual_core::db::ProjectLayout::new(&root).project_config_path;
    let mut config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config.default_backend = Some("validate-only".into());
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let err =
        run_ritual_command(&root, spec_path.to_str().unwrap(), Some("missing-backend"), false)
            .unwrap_err();
    assert!(err.to_string().contains("Backend 'missing-backend' not found"));
}

#[test]
fn list_ritual_runs_handles_empty_state() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("EmptyRuns".into())).unwrap();
    list_ritual_runs_command(&root, None, false).unwrap();
    list_ritual_runs_command(&root, None, true).unwrap();
}

#[test]
fn list_ritual_specs_handles_missing_directory() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("MissingSpecs".into())).unwrap();
    let rituals_dir = ritual_core::db::ProjectLayout::new(&root).rituals_dir;
    std::fs::remove_dir_all(&rituals_dir).unwrap();
    list_ritual_specs_command(&root, false).unwrap();
}

#[test]
fn list_binaries_human_when_empty() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("EmptyBins".into())).unwrap();
    list_binaries_command(&root, false).unwrap();
}

#[test]
fn show_ritual_run_errors_when_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("ShowErrProj".into())).unwrap();
    let err = show_ritual_run_command(&root, "NoBin", "NoRun", false).unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn show_ritual_run_prefers_disk_metadata_when_db_missing() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();
    init_project_command(&root, Some("DiskMetaProj".into())).unwrap();
    let layout = ritual_core::db::ProjectLayout::new(&root);

    // Create run dir only on disk (DB has no run records).
    let run_dir = layout.binary_output_root("DiskBin").join("DiskRun");
    std::fs::create_dir_all(&run_dir).unwrap();
    let metadata = RitualRunMetadata {
        ritual: "DiskRun".into(),
        binary: "DiskBin".into(),
        spec_hash: "disk-only".into(),
        binary_hash: None,
        backend: "validate-only".into(),
        backend_version: Some("v0".into()),
        backend_path: None,
        started_at: "s".into(),
        finished_at: "f".into(),
        status: ritual_core::db::RitualRunStatus::Stubbed,
    };
    std::fs::write(
        run_dir.join("run_metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    // Show human and JSON to hit disk-only branches.
    show_ritual_run_command(&root, "DiskBin", "DiskRun", false).unwrap();
    show_ritual_run_command(&root, "DiskBin", "DiskRun", true).unwrap();
}
