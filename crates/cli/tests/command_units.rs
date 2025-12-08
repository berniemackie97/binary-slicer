use binary_slicer::commands::{
    collect_ritual_runs_on_disk, collect_ritual_specs, sha256_bytes, validate_run_status,
    RitualRunMetadata, RitualSpec,
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
