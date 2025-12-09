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
    assert_eq!(v["ritual_specs"].as_array().unwrap().len(), 0);
    assert_eq!(v["ritual_runs"].as_array().unwrap().len(), 0);
    assert!(!v["available_backends"].as_array().unwrap().is_empty());
}

/// `project-info --json` should include runs/specs when present.
#[test]
fn project_info_json_includes_runs() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libInfo.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("InfoBin")
        .assert()
        .success();

    let spec_path = root.join("info.yaml");
    let spec_yaml = r#"name: InfoRun
binary: InfoBin
roots: [start]
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
    let v: serde_json::Value = serde_json::from_slice(&output).expect("parse json");
    assert_eq!(v["ritual_runs"].as_array().unwrap().len(), 1);
    assert_eq!(v["ritual_runs"][0]["binary"], "InfoBin");
    assert_eq!(v["ritual_runs"][0]["name"], "InfoRun");
}

/// `update-ritual-run-status` should validate and persist status updates.
#[test]
fn update_ritual_run_status_updates_db() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libStatus.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("StatusBin")
        .assert()
        .success();

    let spec_path = root.join("status.yaml");
    let spec_yaml = r#"name: StatusRun
binary: StatusBin
roots: [entry_point]
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

    // Update status to succeeded.
    cargo_bin_cmd!("binary-slicer")
        .arg("update-ritual-run-status")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("StatusBin")
        .arg("--ritual")
        .arg("StatusRun")
        .arg("--status")
        .arg("succeeded")
        .assert()
        .success();

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-runs")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("StatusBin")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output).expect("parse runs json");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["status"], "succeeded");
}

/// `update-ritual-run-status` should reject invalid statuses.
#[test]
fn update_ritual_run_status_rejects_invalid_status() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();
    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libStatus2.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("StatusBin2")
        .assert()
        .success();

    let spec_path = root.join("status2.yaml");
    let spec_yaml = r#"name: StatusRun2
binary: StatusBin2
roots: [entry_point]
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

    cargo_bin_cmd!("binary-slicer")
        .arg("update-ritual-run-status")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("StatusBin2")
        .arg("--ritual")
        .arg("StatusRun2")
        .arg("--status")
        .arg("not-a-status")
        .assert()
        .failure();
}

/// `rerun-ritual` should reuse a normalized spec and create a new run entry.
#[test]
fn rerun_ritual_creates_new_run_from_existing_spec() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libRerun.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("RerunBin")
        .assert()
        .success();

    let spec_path = root.join("orig.yaml");
    let spec_yaml = r#"name: OrigRun
binary: RerunBin
roots: [entry_point]
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

    // Rerun using existing spec, to a new name.
    cargo_bin_cmd!("binary-slicer")
        .arg("rerun-ritual")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("RerunBin")
        .arg("--ritual")
        .arg("OrigRun")
        .arg("--as-name")
        .arg("SecondRun")
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    let new_run_root = layout.binary_output_root("RerunBin").join("SecondRun");
    assert!(new_run_root.join("spec.yaml").is_file());
    assert!(new_run_root.join("report.json").is_file());
    assert!(new_run_root.join("run_metadata.json").is_file());

    // Run list should include both runs.
    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-runs")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("RerunBin")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output).expect("parse runs json");
    assert_eq!(runs.len(), 2);
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

    let bin_path = root.join("libExampleGame.so");
    fs::write(&bin_path, b"dummy").expect("write binary");

    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("ExampleBin")
        .assert()
        .success();

    // Write ritual spec (YAML).
    let spec_path = root.join("ritual.yaml");
    let spec_yaml = r#"name: SliceRun
binary: ExampleBin
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
    let run_root = layout.binary_output_root("ExampleBin").join("SliceRun");
    let spec_out = run_root.join("spec.yaml");
    let report_out = run_root.join("report.json");

    let spec_contents = fs::read_to_string(&spec_out).expect("read normalized spec");
    let spec: serde_yaml::Value = serde_yaml::from_str(&spec_contents).expect("parse spec yaml");
    assert_eq!(spec["name"], "SliceRun");
    assert_eq!(spec["binary"], "ExampleBin");
    assert_eq!(spec["roots"][0], "main_loop");

    let report_contents = fs::read_to_string(&report_out).expect("read report");
    let report_json: serde_json::Value =
        serde_json::from_str(&report_contents).expect("parse report json");
    assert_eq!(report_json["binary"], "ExampleBin");
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

/// `run-ritual` should fail when an output dir exists unless --force is used.
#[test]
fn run_ritual_force_overwrites_existing_run() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libForce.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("ForceBin")
        .assert()
        .success();

    let spec_path = root.join("force.yaml");
    let spec_yaml = r#"name: ForceRun
binary: ForceBin
roots: [entry_point]
"#;
    fs::write(&spec_path, spec_yaml).expect("write spec");

    // First run succeeds.
    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .success();

    // Second run without force should fail.
    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .failure();

    // Third run with --force should overwrite.
    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .arg("--force")
        .assert()
        .success();
}

/// `run-ritual` should emit run_metadata.json with hashes and timestamps.
#[test]
fn run_ritual_writes_metadata_with_hashes() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libMeta.so");
    fs::write(&bin_path, b"payload").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("MetaBin")
        .assert()
        .success();

    let spec_path = root.join("meta.yaml");
    let spec_yaml = r#"name: MetaRun
binary: MetaBin
roots: [entry_point]
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
    let run_root = layout.binary_output_root("MetaBin").join("MetaRun");
    let metadata_path = run_root.join("run_metadata.json");
    let contents = fs::read_to_string(&metadata_path).expect("read run metadata");
    let meta: serde_json::Value = serde_json::from_str(&contents).expect("parse metadata");

    // Check hashes and timestamps exist.
    let mut hasher = Sha256::new();
    hasher.update(spec_yaml.as_bytes());
    let expected_spec_hash = format!("{:x}", hasher.finalize());
    assert_eq!(meta["spec_hash"], expected_spec_hash);
    assert_eq!(meta["binary"], "MetaBin");
    assert_eq!(meta["ritual"], "MetaRun");
    assert!(meta["binary_hash"].as_str().is_some());
    assert!(meta["started_at"].as_str().is_some());
    assert!(meta["finished_at"].as_str().is_some());

    // Graph artifact should exist (DOT).
    let dot_path = run_root.join("graph.dot");
    assert!(dot_path.is_file(), "graph.dot should be emitted");
}

/// `show-ritual-run` should print metadata and support JSON.
#[test]
fn show_ritual_run_reports_metadata() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libShow.so");
    fs::write(&bin_path, b"payload").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("ShowBin")
        .assert()
        .success();

    let spec_path = root.join("show.yaml");
    let spec_yaml = r#"name: ShowRun
binary: ShowBin
roots: [entry_point]
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

    // Human output
    cargo_bin_cmd!("binary-slicer")
        .arg("show-ritual-run")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("ShowBin")
        .arg("--ritual")
        .arg("ShowRun")
        .assert()
        .success()
        .stdout(predicate::str::contains("ShowBin"))
        .stdout(predicate::str::contains("ShowRun"))
        .stdout(predicate::str::contains("run_metadata.json").not()); // we don't print the filename literally

    // JSON output
    let output = cargo_bin_cmd!("binary-slicer")
        .arg("show-ritual-run")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("ShowBin")
        .arg("--ritual")
        .arg("ShowRun")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: serde_json::Value =
        serde_json::from_slice(&output).expect("parse show-ritual-run json");
    assert_eq!(payload["binary"], "ShowBin");
    assert_eq!(payload["ritual"], "ShowRun");
    assert!(payload["metadata"]["spec_hash"].as_str().is_some());
}

/// `show-ritual-run` should prefer DB metadata when available.
#[test]
fn show_ritual_run_uses_db_metadata() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libShowDb.so");
    fs::write(&bin_path, b"payload").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("ShowDbBin")
        .assert()
        .success();

    let spec_path = root.join("showdb.yaml");
    let spec_yaml = r#"name: ShowDbRun
binary: ShowDbBin
roots: [entry_point]
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

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("show-ritual-run")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("ShowDbBin")
        .arg("--ritual")
        .arg("ShowDbRun")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: serde_json::Value = serde_json::from_slice(&output).expect("parse show json");
    assert_eq!(payload["metadata"]["status"], "stubbed");
    assert_eq!(payload["binary"], "ShowDbBin");
    assert_eq!(payload["ritual"], "ShowDbRun");
}

/// `list-ritual-specs` should report specs under rituals/.
#[test]
fn list_ritual_specs_reports_specs() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let rituals_dir = ritual_core::db::ProjectLayout::new(root).rituals_dir;
    let spec_path = rituals_dir.join("listed.yaml");
    let spec_yaml = r#"name: ListedRun
binary: ListedBin
roots: [main]
"#;
    fs::write(&spec_path, spec_yaml).expect("write spec");

    cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-specs")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("ListedRun"))
        .stdout(predicate::str::contains("ListedBin"));

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-specs")
        .arg("--root")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let specs: Vec<serde_json::Value> = serde_json::from_slice(&output).expect("parse specs json");
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0]["name"], "ListedRun");
    assert_eq!(specs[0]["binary"], "ListedBin");
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

/// `list-ritual-runs` should enumerate runs under outputs/binaries.
#[test]
fn list_ritual_runs_reports_runs() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libList.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("ListBin")
        .assert()
        .success();

    let spec_path = root.join("list.yaml");
    let spec_yaml = r#"name: ListRun
binary: ListBin
roots: [start]
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

    cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-runs")
        .arg("--root")
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("ListBin"))
        .stdout(predicate::str::contains("ListRun"));

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-runs")
        .arg("--root")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output).expect("parse runs json");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["binary"], "ListBin");
    assert_eq!(runs[0]["name"], "ListRun");
}

/// `list-ritual-runs --json` should include DB metadata.
#[test]
fn list_ritual_runs_json_includes_status() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    let bin_path = root.join("libJson.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("JsonBin")
        .assert()
        .success();

    let spec_path = root.join("jsonrun.yaml");
    let spec_yaml = r#"name: JsonRun
binary: JsonBin
roots: [entry_point]
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

    let output = cargo_bin_cmd!("binary-slicer")
        .arg("list-ritual-runs")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("JsonBin")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output).expect("parse runs json");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["binary"], "JsonBin");
    assert_eq!(runs[0]["name"], "JsonRun");
    assert_eq!(runs[0]["status"], "stubbed");
    assert!(runs[0]["spec_hash"].as_str().is_some());
}

/// `clean-outputs` should refuse without --yes and delete scoped outputs with it.
#[test]
fn clean_outputs_requires_confirmation_and_scopes() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();

    // Create two binaries and runs.
    for (name, spec_name) in [("BinA", "RunA"), ("BinB", "RunB")] {
        let bin_path = root.join(format!("{name}.so"));
        fs::write(&bin_path, b"dummy").expect("write binary");
        cargo_bin_cmd!("binary-slicer")
            .arg("add-binary")
            .arg("--root")
            .arg(root)
            .arg("--path")
            .arg(&bin_path)
            .arg("--name")
            .arg(name)
            .assert()
            .success();
        let spec_path = root.join(format!("{spec_name}.yaml"));
        let spec_yaml = format!(
            "name: {spec_name}\nbinary: {name}\nroots: [entry_point]\n",
            spec_name = spec_name,
            name = name
        );
        fs::write(&spec_path, spec_yaml).expect("write spec");
        cargo_bin_cmd!("binary-slicer")
            .arg("run-ritual")
            .arg("--root")
            .arg(root)
            .arg("--file")
            .arg(&spec_path)
            .assert()
            .success();
    }

    let layout = ritual_core::db::ProjectLayout::new(root);
    let bin_a_run = layout.binary_output_root("BinA").join("RunA");
    let bin_b_run = layout.binary_output_root("BinB").join("RunB");
    assert!(bin_a_run.is_dir());
    assert!(bin_b_run.is_dir());

    // Without --yes should fail fast.
    cargo_bin_cmd!("binary-slicer")
        .arg("clean-outputs")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("BinA")
        .assert()
        .failure();

    // With --yes should delete BinA/RunA only.
    cargo_bin_cmd!("binary-slicer")
        .arg("clean-outputs")
        .arg("--root")
        .arg(root)
        .arg("--binary")
        .arg("BinA")
        .arg("--yes")
        .assert()
        .success();
    assert!(!bin_a_run.exists());
    assert!(bin_b_run.exists());
}

/// `clean-outputs --all` should delete all outputs when confirmed.
#[test]
fn clean_outputs_all_removes_everything() {
    let temp = tempdir().expect("temp dir");
    let root = temp.path();

    cargo_bin_cmd!("binary-slicer").arg("init-project").arg("--root").arg(root).assert().success();
    let bin_path = root.join("libAll.so");
    fs::write(&bin_path, b"dummy").expect("write binary");
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("AllBin")
        .assert()
        .success();
    let spec_path = root.join("all.yaml");
    fs::write(&spec_path, "name: AllRun\nbinary: AllBin\nroots: [start]\n").expect("write spec");
    cargo_bin_cmd!("binary-slicer")
        .arg("run-ritual")
        .arg("--root")
        .arg(root)
        .arg("--file")
        .arg(&spec_path)
        .assert()
        .success();

    let layout = ritual_core::db::ProjectLayout::new(root);
    assert!(layout.binary_output_root("AllBin").join("AllRun").is_dir());

    cargo_bin_cmd!("binary-slicer")
        .arg("clean-outputs")
        .arg("--root")
        .arg(root)
        .arg("--all")
        .arg("--yes")
        .assert()
        .success();
    // Outputs dir should be gone or empty.
    if layout.outputs_binaries_dir.exists() {
        assert!(layout.outputs_binaries_dir.read_dir().unwrap().next().is_none());
    }
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
