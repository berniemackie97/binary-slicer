use std::fs;

use ritual_core::db::ProjectLayout;
use tempfile::tempdir;

/// Running the CLI with no arguments should default to the Hello command
/// and succeed.
#[test]
fn hello_default_command_runs_successfully() {
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli").assert().success();
}

/// Hello with an explicit slice name should also succeed.
#[test]
fn hello_named_slice_command_runs_successfully() {
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("hello")
        .arg("--slice")
        .arg("CustomSlice")
        .assert()
        .success();
}

/// init-project without an explicit --root should use the current directory
/// as the project root and write the config file.
///
/// The DB file itself is created lazily when the core opens it, which is
/// covered by other tests (core/db_integration and cli/init_and_add_binary).
#[test]
fn init_project_uses_default_root_when_not_provided() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Run `ritual-cli init-project --name TestProject` with CWD = root.
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .current_dir(root)
        .arg("init-project")
        .arg("--name")
        .arg("TestProject")
        .assert()
        .success();

    let layout = ProjectLayout::new(root);

    assert!(
        layout.project_config_path.exists(),
        "project config should exist at {}",
        layout.project_config_path.display()
    );

    // We *do not* require the DB file to exist yet here; that happens on the
    // first DB-dependent operation (e.g., add-binary), which is tested in
    // `init_and_add_binary`.
}

/// project-info should fail (non-zero exit) if no project config exists.
#[test]
fn project_info_fails_when_config_missing() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("project-info")
        .arg("--root")
        .arg(root)
        .assert()
        .failure();
}

/// add-binary should fail when the target file does not exist.
#[test]
fn add_binary_fails_for_missing_file() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // First, create a project so we have a config and DB.
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("TestProject")
        .assert()
        .success();

    // Then attempt to add a non-existent binary.
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg("nonexistent.bin")
        .assert()
        .failure();
}

/// init-slice should succeed after init-project and create a slice doc
/// under docs/slices/<Name>.md.
#[test]
fn init_slice_scaffolds_slice_doc() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Initialize a project at this root.
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("SliceProject")
        .assert()
        .success();

    // 2. Initialize a slice.
    assert_cmd::cargo::cargo_bin_cmd!("ritual-cli")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("AutoUpdateManager")
        .arg("--description")
        .arg("Auto-update subsystem for CQ2ClientLib")
        .assert()
        .success();

    // 3. Verify that the doc exists where we expect it.
    let layout = ProjectLayout::new(root);
    let doc_path = layout.slices_docs_dir.join("AutoUpdateManager.md");
    assert!(doc_path.exists(), "slice doc should be created at {}", doc_path.display());

    // Sanity check: file is non-empty and has the title.
    let contents = fs::read_to_string(doc_path).expect("read slice doc");
    assert!(
        contents.contains("Slice: AutoUpdateManager"),
        "slice doc should contain a title header"
    );
}
