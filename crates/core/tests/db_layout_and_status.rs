use ritual_core::db::{ProjectLayout, SliceRecord, SliceStatus};
use std::path::{PathBuf, MAIN_SEPARATOR};

#[test]
fn project_layout_uses_expected_paths() {
    let layout = ProjectLayout::new("/my/project");

    assert_eq!(layout.root, PathBuf::from("/my/project"));
    assert_eq!(layout.meta_dir, PathBuf::from("/my/project/.ritual"));
    assert_eq!(layout.project_config_path, PathBuf::from("/my/project/.ritual/project.json"));
    assert_eq!(layout.db_path, PathBuf::from("/my/project/.ritual/project.db"));
    assert_eq!(layout.docs_dir, PathBuf::from("/my/project/docs"));
    assert_eq!(layout.slices_docs_dir, PathBuf::from("/my/project/docs/slices"));
    assert_eq!(layout.reports_dir, PathBuf::from("/my/project/reports"));
    assert_eq!(layout.graphs_dir, PathBuf::from("/my/project/graphs"));
    assert_eq!(layout.rituals_dir, PathBuf::from("/my/project/rituals"));
    assert_eq!(layout.outputs_dir, PathBuf::from("/my/project/outputs"));
    assert_eq!(layout.outputs_binaries_dir, PathBuf::from("/my/project/outputs/binaries"));

    let bin_root = layout.binary_output_root("libExampleGame.so");
    assert_eq!(bin_root, PathBuf::from("/my/project/outputs/binaries/libExampleGame.so"));

    // Relative string should drop the root prefix when possible.
    let expected_rel = format!(".ritual{}project.db", MAIN_SEPARATOR);
    assert_eq!(layout.db_path_relative_string(), expected_rel);
}

#[test]
fn slice_record_and_status_round_trip() {
    let slice = SliceRecord::new("AutoUpdateManager", SliceStatus::Active);
    let json = serde_json::to_string(&slice).unwrap();
    let deserialized: SliceRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "AutoUpdateManager");
    assert_eq!(deserialized.status, SliceStatus::Active);

    // Explicitly test status mapping.
    assert_eq!(SliceStatus::Planned.to_i32(), 0);
    assert_eq!(SliceStatus::Draft.to_i32(), 1);
    assert_eq!(SliceStatus::Active.to_i32(), 2);
    assert_eq!(SliceStatus::Deprecated.to_i32(), 3);
    assert_eq!(SliceStatus::from_i32(0), SliceStatus::Planned);
    assert_eq!(SliceStatus::from_i32(1), SliceStatus::Draft);
    assert_eq!(SliceStatus::from_i32(2), SliceStatus::Active);
    assert_eq!(SliceStatus::from_i32(3), SliceStatus::Deprecated);
    // Unknown values fall back to Draft.
    assert_eq!(SliceStatus::from_i32(99), SliceStatus::Draft);
}
