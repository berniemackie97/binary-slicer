use ritual_core::db::{
    BinaryRecord, ProjectConfig, ProjectLayout, ProjectSnapshot, SliceRecord, SliceStatus,
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
