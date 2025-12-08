use ritual_core::db::ProjectLayout;

#[test]
fn db_path_relative_string_prefers_relative() {
    let root = tempfile::tempdir().unwrap();
    let layout = ProjectLayout::new(root.path());
    let rel = layout.db_path_relative_string();
    assert!(rel.starts_with(".ritual"));
}

#[test]
fn binary_output_root_appends_name() {
    let root = tempfile::tempdir().unwrap();
    let layout = ProjectLayout::new(root.path());
    let out = layout.binary_output_root("GameBin");
    assert!(out.ends_with("outputs/binaries/GameBin"));
}
