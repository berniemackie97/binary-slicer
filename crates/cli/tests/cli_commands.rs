use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::tempdir;

use ritual_core::db::{ProjectDb, ProjectLayout};

/// Running the binary with no arguments should execute the default
/// Hello command (which uses the default slice name).
#[test]
fn hello_default_command_runs_successfully() {
    cargo_bin_cmd!("ritual-cli").assert().success();
}

/// Explicit Hello command with a named slice should also succeed.
#[test]
fn hello_named_slice_command_runs_successfully() {
    cargo_bin_cmd!("ritual-cli")
        .arg("hello")
        .arg("--slice")
        .arg("AutoUpdateManager")
        .assert()
        .success();
}

/// When no --root is provided, init-project should treat "." as the root,
/// which we simulate by setting the command's current_dir to a temp directory.
/// The CLI writes project config and creates the directory layout; the DB is
/// created lazily when first opened.
#[test]
fn init_project_uses_default_root_when_not_provided() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // current_dir(root) means inside the CLI, "." will resolve to this temp dir.
    cargo_bin_cmd!("ritual-cli")
        .current_dir(root)
        .arg("init-project")
        .arg("--name")
        .arg("DefaultRootProject")
        .assert()
        .success();

    let layout = ProjectLayout::new(root);

    // Config should be written by init-project.
    assert!(layout.project_config_path.exists(), "project config should exist after init-project");

    // DB is created lazily when opened.
    let db = ProjectDb::open(&layout.db_path).expect("open db");
    assert!(layout.db_path.exists(), "project DB should exist after opening via ProjectDb");

    let binaries = db.list_binaries().expect("list binaries");
    assert!(binaries.is_empty(), "newly initialized project should have no binaries");
}

/// project-info should fail (non-zero exit) if there is no .ritual/project.json
/// for the given root.
#[test]
fn project_info_fails_when_config_missing() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    cargo_bin_cmd!("ritual-cli").arg("project-info").arg("--root").arg(root).assert().failure();
}

/// add-binary should fail (non-zero exit) if the referenced binary file does not exist.
#[test]
fn add_binary_fails_for_missing_file() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // First initialize a project so config + DB exist.
    cargo_bin_cmd!("ritual-cli")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("MissingBinaryProject")
        .assert()
        .success();

    // Now attempt to add a non-existent binary under that project.
    let missing_path = root.join("nonexistent-binary.so");

    cargo_bin_cmd!("ritual-cli")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&missing_path)
        .arg("--name")
        .arg("Nonexistent")
        .assert()
        .failure();
}
