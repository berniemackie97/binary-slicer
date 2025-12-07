use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use tempfile::tempdir;

use ritual_core::db::{ProjectDb, ProjectLayout};
use sha2::{Digest, Sha256};

#[test]
fn init_project_and_add_binary_registers_in_db() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Initialize project.
    cargo_bin_cmd!("binary-slicer")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("TestProject")
        .assert()
        .success();

    // 2. Run project-info just to ensure it works and sees the project.
    cargo_bin_cmd!("binary-slicer").arg("project-info").arg("--root").arg(root).assert().success();

    // 3. Create a dummy binary file under the project root.
    let bin_path = root.join("libCQ2Client.so");
    fs::write(&bin_path, b"dummy-binary").expect("write dummy binary");

    // 4. Register the binary via CLI.
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("CQ2ClientLib")
        .arg("--arch")
        .arg("armv7")
        .assert()
        .success();

    // 5. Open the DB directly and verify that the binary was registered.
    let layout = ProjectLayout::new(root);
    let db = ProjectDb::open(&layout.db_path).expect("open db");
    let binaries = db.list_binaries().expect("list binaries");
    assert_eq!(binaries.len(), 1);
    assert_eq!(binaries[0].name, "CQ2ClientLib");
    // Path should be relative to the project root (just the file name).
    assert_eq!(binaries[0].path, "libCQ2Client.so");
    assert_eq!(binaries[0].arch.as_deref(), Some("armv7"));

    let mut hasher = Sha256::new();
    hasher.update(b"dummy-binary");
    let expected_hash = format!("{:x}", hasher.finalize());
    assert_eq!(binaries[0].hash.as_deref(), Some(expected_hash.as_str()));
}
