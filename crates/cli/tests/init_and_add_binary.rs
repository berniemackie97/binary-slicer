use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

use ritual_core::db::{ProjectDb, ProjectLayout};

#[test]
fn init_project_and_add_binary_registers_in_db() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Initialize project.
    cargo_bin_cmd!("ritual-cli")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("TestProject")
        .assert()
        .success();

    // 2. Run project-info just to ensure it works and sees the project.
    cargo_bin_cmd!("ritual-cli").arg("project-info").arg("--root").arg(root).assert().success();

    // 3. Create a dummy binary file under the project root.
    let bin_path = root.join("libCQ2Client.so");
    fs::write(&bin_path, b"dummy-binary").expect("write dummy binary");

    // 4. Register the binary via CLI.
    cargo_bin_cmd!("ritual-cli")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("CQ2ClientLib")
        .assert()
        .success();

    // 5. Open the DB directly and verify that the binary was registered.
    let layout = ProjectLayout::new(root);
    let db = ProjectDb::open(&layout.db_path).expect("open db");
    let binaries = db.list_binaries().expect("list binaries");
    assert_eq!(binaries.len(), 1);
    assert_eq!(binaries[0].name, "CQ2ClientLib");
    // Path should be relative to the project root (just the file name).
    assert!(
        binaries[0].path.ends_with("libCQ2Client.so"),
        "expected relative path to end with libCQ2Client.so, got {:?}",
        binaries[0].path
    );
}
