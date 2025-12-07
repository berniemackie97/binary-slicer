// crates/cli/tests/cli_commands.rs

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

/// `binary-slicer hello` with default slice name.
#[test]
fn hello_default_command_runs_successfully() {
    cargo_bin_cmd!("binary-slicer")
        .arg("hello")
        .assert()
        .success()
        .stdout(predicate::str::contains("binary-slicer v"))
        .stdout(predicate::str::contains("Hello, slice: DefaultSlice"));
}

/// `binary-slicer hello --slice AutoUpdateManager`
#[test]
fn hello_named_slice_command_runs_successfully() {
    cargo_bin_cmd!("binary-slicer")
        .arg("hello")
        .arg("--slice")
        .arg("AutoUpdateManager")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, slice: AutoUpdateManager"));
}

/// Running with no args should default to Hello command.
#[test]
fn default_command_runs_hello() {
    cargo_bin_cmd!("binary-slicer")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, slice: DefaultSlice"));
}

/// `init-project` should work when we omit `--root` and just rely on `.`
/// (using the current working directory).
#[test]
fn init_project_uses_default_root_when_not_provided() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path().to_path_buf();

    // Change cwd to the temp root so that the CLI's default `.` root
    // corresponds to this directory.
    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&root).expect("chdir to temp root");

    cargo_bin_cmd!("binary-slicer")
        .arg("init-project")
        .arg("--name")
        .arg("DefaultRootProject")
        .assert()
        .success();

    // Restore cwd so the rest of the test process behaves normally.
    std::env::set_current_dir(&original_cwd).expect("restore cwd");

    // Now inspect the layout based on the temp root and confirm the DB exists.
    let layout = ritual_core::db::ProjectLayout::new(&root);
    assert!(layout.db_path.is_file(), "project DB should exist at {}", layout.db_path.display());
}

/// `init-project` should derive a name when `--name` is omitted and create the DB.
#[test]
fn init_project_infers_name_and_creates_db_when_root_missing() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path().join("InferThisName");

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(&root).assert().success();

    let layout = ritual_core::db::ProjectLayout::new(&root);

    let config_json = fs::read_to_string(&layout.project_config_path).expect("read project config");
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).expect("parse project config");

    assert_eq!(config.name, "InferThisName");
    assert!(layout.db_path.is_file(), "project DB should exist at {}", layout.db_path.display());
    assert!(layout.outputs_dir.is_dir(), "outputs dir should exist");
    assert!(layout.rituals_dir.is_dir(), "rituals dir should exist");
}

/// `project-info` should fail (non-zero) if the project config does not exist.
#[test]
fn project_info_fails_when_config_missing() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("project-info").arg("--root").arg(root).assert().failure();
}

/// `project-info` should report missing directories as MISSING when they are absent.
#[test]
fn project_info_reports_missing_directories() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();
    let layout = ritual_core::db::ProjectLayout::new(root);

    // Create only the meta dir and project config; leave docs/reports/graphs missing.
    std::fs::create_dir_all(&layout.meta_dir).expect("create meta dir");
    let config =
        ritual_core::db::ProjectConfig::new("MissingDirs", layout.db_path_relative_string());
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    std::fs::write(&layout.project_config_path, json).expect("write project config");

    cargo_bin_cmd!("binary-slicer")
        .arg("project-info")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Docs dir"))
        .stdout(predicate::str::contains("MISSING"));
}

/// `project-info --json` should emit machine-readable snapshot including layout.
#[test]
fn project_info_json_output() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("project-info")
        .arg("--root")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let v: serde_json::Value = serde_json::from_str(&body).expect("parse json");
    assert!(v["layout"]["rituals_dir"].as_str().unwrap().ends_with("rituals"));
    assert!(v["layout"]["outputs_dir"].as_str().unwrap().ends_with("outputs"));
}

/// `add-binary` should fail when the target binary path does not exist.
#[test]
fn add_binary_fails_for_missing_file() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    // First create a valid project so the config & DB exist.
    cargo_bin_cmd!("binary-slicer")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("MissingBinaryProject")
        .assert()
        .success();

    let missing_path = root.join("does_not_exist.bin");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&missing_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Binary file does not exist"));
}

/// `init-slice` should:
/// - create a slice doc under `docs/slices`,
/// - register a slice row in the DB with Planned status + description.
#[test]
fn init_slice_scaffolds_slice_doc() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    // 1. Init the project.
    cargo_bin_cmd!("binary-slicer")
        .arg("init-project")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("SliceProject")
        .assert()
        .success();

    // 2. Init a slice.
    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("AutoUpdateManager")
        .arg("--description")
        .arg("AutoUpdateManager slice description")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized slice"));

    // 3. Verify slice doc exists and has a heading.
    let layout = ritual_core::db::ProjectLayout::new(root);
    let slice_doc_path = layout.slices_docs_dir.join("AutoUpdateManager.md");

    let contents = fs::read_to_string(&slice_doc_path)
        .unwrap_or_else(|_| panic!("expected slice doc at {}", slice_doc_path.display()));

    assert!(
        contents.contains("# AutoUpdateManager"),
        "slice doc should contain heading; got:\n{}",
        contents
    );

    // 4. Verify slice is registered in DB with Planned status + description.
    let config_json = fs::read_to_string(&layout.project_config_path).expect("read project config");
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).expect("parse project config");

    let db_path = {
        let rel = std::path::Path::new(&config.db.path);
        if rel.is_absolute() {
            rel.to_path_buf()
        } else {
            layout.root.join(rel)
        }
    };

    let db = ritual_core::db::ProjectDb::open(&db_path).expect("open db");
    let slices = db.list_slices().expect("list slices");

    let slice = slices
        .iter()
        .find(|s| s.name == "AutoUpdateManager")
        .unwrap_or_else(|| panic!("expected AutoUpdateManager slice, got {:?}", slices));

    assert_eq!(slice.description.as_deref(), Some("AutoUpdateManager slice description"));
    assert_eq!(slice.status, ritual_core::db::SliceStatus::Planned);
}

/// `list-slices` should show slices registered in the DB.
#[test]
fn list_slices_reports_registered_slice() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    // Init project and slice.
    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("Telemetry")
        .assert()
        .success();

    cargo_bin_cmd!("binary-slicer")
        .arg("list-slices")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("Telemetry"))
        .stdout(predicate::str::contains("Planned"));
}

/// `list-slices` should clearly state when there are no slices.
#[test]
fn list_slices_reports_none_when_empty() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("list-slices")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("(none)"));
}

/// `init-slice` should fail when the slice name already exists.
#[test]
fn init_slice_fails_on_duplicate_name() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("DuplicateSlice")
        .assert()
        .success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("DuplicateSlice")
        .assert()
        .failure();
}

/// `list-binaries` should show registered binaries with arch and hash.
#[test]
fn list_binaries_reports_registered_binary_with_hash() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("gameclient.bin");
    fs::write(&bin_path, b"binary-payload").expect("write binary");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--arch")
        .arg("arm64")
        .assert()
        .success();

    let mut hasher = Sha256::new();
    hasher.update(b"binary-payload");
    let expected_hash = format!("{:x}", hasher.finalize());

    cargo_bin_cmd!("binary-slicer")
        .arg("list-binaries")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("gameclient.bin"))
        .stdout(predicate::str::contains("arch: arm64"))
        .stdout(predicate::str::contains(&expected_hash));
}

/// `emit-slice-docs` should regenerate docs from DB slices.
#[test]
fn emit_slice_docs_regenerates_docs() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("Telemetry")
        .arg("--description")
        .arg("Telemetry slice")
        .assert()
        .success();

    // Delete the doc to ensure regen works.
    let layout = ritual_core::db::ProjectLayout::new(root);
    let doc_path = layout.slices_docs_dir.join("Telemetry.md");
    std::fs::remove_file(&doc_path).expect("delete doc");

    cargo_bin_cmd!("binary-slicer")
        .arg("emit-slice-docs")
        .arg("--root")
        .arg(root)
        .assert()
        .success();

    let contents = std::fs::read_to_string(&doc_path).expect("read doc");
    assert!(contents.contains("Telemetry"));
    assert!(contents.contains("Telemetry slice"));
}

/// `emit-slice-reports` should write JSON reports per slice.
#[test]
fn emit_slice_reports_regenerates_reports() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("Telemetry")
        .arg("--description")
        .arg("Telemetry slice")
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    let report_path = layout.reports_dir.join("Telemetry.json");
    if report_path.exists() {
        std::fs::remove_file(&report_path).expect("delete report");
    }

    cargo_bin_cmd!("binary-slicer")
        .arg("emit-slice-reports")
        .arg("--root")
        .arg(root)
        .assert()
        .success();

    let contents = std::fs::read_to_string(&report_path).expect("read report");
    let v: serde_json::Value = serde_json::from_str(&contents).expect("parse report");
    assert_eq!(v["name"], "Telemetry");
    assert_eq!(v["description"], "Telemetry slice");
    assert_eq!(v["status"], "Planned");
}

/// `emit-slice-docs` should no-op gracefully when there are no slices.
#[test]
fn emit_slice_docs_handles_empty_db() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("emit-slice-docs")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("No slices to emit docs for."));
}

/// `emit-slice-reports` should no-op gracefully when there are no slices.
#[test]
fn emit_slice_reports_handles_empty_db() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("emit-slice-reports")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("No slices to emit reports for."));
}

/// `run-ritual` should parse a spec, ensure binary exists, and write outputs per binary/ritual.
#[test]
fn run_ritual_scaffolds_output_for_binary() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    // Init project and add a binary.
    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libCQ2Client.so");
    fs::write(&bin_path, b"dummy").expect("write binary");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("CQ2Bin")
        .assert()
        .success();

    // Write ritual spec (YAML).
    let spec_path = root.join("ritual.yaml");
    let spec_yaml = r#"name: SliceRun
binary: CQ2Bin
roots:
  - main_loop
"#;
    fs::write(&spec_path, spec_yaml).expect("write spec");

    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    let run_root = layout.binary_output_root("CQ2Bin").join("SliceRun");
    let spec_out = run_root.join("spec.yaml");
    let report_out = run_root.join("report.json");

    let spec_contents = fs::read_to_string(&spec_out).expect("read normalized spec");
    let spec: serde_yaml::Value = serde_yaml::from_str(&spec_contents).expect("parse spec yaml");
    assert_eq!(spec["name"], "SliceRun");
    assert_eq!(spec["binary"], "CQ2Bin");
    assert_eq!(spec["roots"][0], "main_loop");

    let report_contents = fs::read_to_string(&report_out).expect("read report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report_contents).expect("parse report json");
    assert_eq!(report_json["binary"], "CQ2Bin");
    assert_eq!(report_json["ritual"], "SliceRun");
}

/// `run-ritual` should handle JSON specs too.
#[test]
fn run_ritual_accepts_json_spec() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libGame.so");
    fs::write(&bin_path, b"dummy").expect("write binary");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("GameBin")
        .assert()
        .success();

    let spec_path = root.join("ritual.json");
    let spec_json = serde_json::json!({
        "name": "JsonRun",
        "binary": "GameBin",
        "roots": ["entry_point"],
        "max_depth": 2
    });
    fs::write(&spec_path, serde_json::to_string_pretty(&spec_json).unwrap()).expect("write spec");

    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    let run_root = layout.binary_output_root("GameBin").join("JsonRun");
    assert!(run_root.is_dir());
    assert!(run_root.join("report.json").is_file());
}

/// `run-ritual` should fail validation when roots are missing.
#[test]
fn run_ritual_rejects_missing_roots() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libBad.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("BadBin")
        .assert()
        .success();

    let spec_path = root.join("invalid.yaml");
    let spec_yaml = r#"name: Invalid
binary: BadBin
roots: []
"#;
    fs::write(&spec_path, spec_yaml).expect("write spec");

    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one root"));
}

/// `run-ritual` should fail when the binary is not registered.
#[test]
fn run_ritual_errors_when_binary_missing() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();
    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let spec_path = root.join("missing.yaml");
    let spec_yaml = r#"name: MissingBinRun
binary: NotThere
roots: [root_fn]
"#;
    fs::write(&spec_path, spec_yaml).expect("write spec");

    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .failure();
}

/// `add-binary` should accept a precomputed hash and skip hashing when requested.
#[test]
fn add_binary_respects_provided_hash_and_skip_flag() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("demo.bin");
    fs::write(&bin_path, b"payload-1").expect("write binary");

    // Provide an explicit hash.
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--hash")
        .arg("precomputed")
        .assert()
        .success();

    // Add another binary with skip-hash.
    let bin_path2 = root.join("demo2.bin");
    fs::write(&bin_path2, b"payload-2").expect("write binary2");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path2)
        .arg("--skip-hash")
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    let config_json = fs::read_to_string(&layout.project_config_path).expect("read config");
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).expect("parse config");
    let db_path = layout.root.join(config.db.path);
    let db = ritual_core::db::ProjectDb::open(&db_path).expect("open db");
    let binaries = db.list_binaries().expect("list binaries");

    assert_eq!(binaries.len(), 2);
    let first = binaries.iter().find(|b| b.path.ends_with("demo.bin")).unwrap();
    assert_eq!(first.hash.as_deref(), Some("precomputed"));

    let second = binaries.iter().find(|b| b.path.ends_with("demo2.bin")).unwrap();
    assert!(second.hash.is_none(), "expected skip-hash to store None");
}

/// `list-slices --json` should emit JSON.
#[test]
fn list_slices_json_output() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    cargo_bin_cmd!("binary-slicer")
        .arg("init-slice")
        .arg("--root")
        .arg(root)
        .arg("--name")
        .arg("Telemetry")
        .arg("--description")
        .arg("Slice desc")
        .assert()
        .success();

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-slices")
        .arg("--root")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let slices: Vec<ritual_core::db::SliceRecord> =
        serde_json::from_str(&body).expect("parse slices json");
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0].name, "Telemetry");
    assert_eq!(slices[0].description.as_deref(), Some("Slice desc"));
}

/// `list-binaries --json` should emit JSON with hash values.
#[test]
fn list_binaries_json_output() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("client.bin");
    fs::write(&bin_path, b"abc").expect("write binary");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .assert()
        .success();

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-binaries")
        .arg("--root")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let body = String::from_utf8(output).expect("utf8");
    let binaries: Vec<ritual_core::db::BinaryRecord> =
        serde_json::from_str(&body).expect("parse binaries json");
    assert_eq!(binaries.len(), 1);
    assert_eq!(binaries[0].name, "client.bin");
    assert!(binaries[0].hash.as_ref().is_some());
}
