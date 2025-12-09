use ritual_core::db::{
    BinaryRecord, ProjectConfig, ProjectLayout, ProjectSnapshot, RitualRunRecord, SliceRecord,
    SliceStatus,
};

/// Ensure BinaryRecord::new sets optional fields to None.
#[test]
fn binary_record_new_defaults() {
    let bin = BinaryRecord::new("name", "path/to/bin");
    assert_eq!(bin.name, "name");
    assert_eq!(bin.path, "path/to/bin");
    assert!(bin.arch.is_none());
    assert!(bin.hash.is_none());
}

/// Ensure SliceRecord builder sets description.
#[test]
fn slice_record_builder_sets_description() {
    let slice =
        SliceRecord::new("Telemetry", SliceStatus::Draft).with_description(Some("desc".into()));
    assert_eq!(slice.name, "Telemetry");
    assert_eq!(slice.status, SliceStatus::Draft);
    assert_eq!(slice.description.as_deref(), Some("desc"));
}

/// ProjectSnapshot should serialize/deserialize as expected.
#[test]
fn project_snapshot_round_trip() {
    let layout = ProjectLayout::new("/tmp/project");
    let config = ProjectConfig::new("proj", layout.db_path_relative_string());

    let snapshot = ProjectSnapshot {
        config: config.clone(),
        binaries: vec![BinaryRecord::new("bin", "binaries/bin.so")],
        slices: vec![SliceRecord::new("Telemetry", SliceStatus::Planned)],
    };

    let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let de: ProjectSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");
    assert_eq!(de.config.name, config.name);
    assert_eq!(de.binaries.len(), 1);
    assert_eq!(de.slices.len(), 1);
}

/// ProjectConfig should tolerate optional default_backend.
#[test]
fn project_config_default_backend_round_trip() {
    let mut config = ProjectConfig::new("proj", "db.sqlite");
    config.default_backend = Some("validate-only".into());
    let json = serde_json::to_string(&config).expect("serialize config");
    let de: ProjectConfig = serde_json::from_str(&json).expect("deserialize config");
    assert_eq!(de.default_backend.as_deref(), Some("validate-only"));

    // Missing field should default to None.
    let minimal = r#"{ "name": "proj", "config_version": "0.1.0", "db": { "path": "db.sqlite" } }"#;
    let de2: ProjectConfig = serde_json::from_str(minimal).expect("deserialize minimal config");
    assert!(de2.default_backend.is_none());
}

/// RitualRunRecord should round-trip via serde.
#[test]
fn ritual_run_record_round_trip() {
    let record = RitualRunRecord {
        binary: "Bin".into(),
        ritual: "Rit".into(),
        spec_hash: "abc".into(),
        binary_hash: Some("binhash".into()),
        backend: "validate-only".into(),
        status: ritual_core::db::RitualRunStatus::Stubbed,
        started_at: "now".into(),
        finished_at: "now".into(),
    };
    let json = serde_json::to_string(&record).expect("serialize run");
    let de: RitualRunRecord = serde_json::from_str(&json).expect("deserialize run");
    assert_eq!(de.binary, "Bin");
    assert_eq!(de.ritual, "Rit");
    assert_eq!(de.binary_hash.as_deref(), Some("binhash"));
}
