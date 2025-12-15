use assert_cmd::cargo::cargo_bin_cmd;
use binary_slicer::commands::init_project_command;
use ritual_core::db::{ProjectDb, ProjectLayout, RitualRunRecord, RitualRunStatus};
use ritual_core::services::analysis::{
    AnalysisResult, BasicBlock, BlockEdge, BlockEdgeKind, CallEdge, EvidenceRecord, FunctionRecord,
};
use serde_json::{self, Value};
use tempfile::tempdir;

#[test]
fn show_ritual_run_json_includes_persisted_analysis_counts() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();

    // Initialize a project and seed a ritual run + analysis directly into the DB.
    init_project_command(&root, Some("AnalysisProj".into())).unwrap();
    let layout = ProjectLayout::new(&root);
    let db = ProjectDb::open(&layout.db_path).expect("open db");

    let run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "RunOne".into(),
        spec_hash: "spec123".into(),
        binary_hash: Some("binhash123".into()),
        backend: "rizin".into(),
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: RitualRunStatus::Succeeded,
        started_at: "t0".into(),
        finished_at: "t1".into(),
    };
    let run_id = db.insert_ritual_run(&run).expect("insert run");

    let analysis = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x1000,
            name: Some("func_main".into()),
            size: Some(16),
            in_slice: false,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x1000, to: 0x2000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x1000,
            len: 8,
            successors: vec![BlockEdge { target: 0x2000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord {
            address: 0x1000,
            description: "test-evidence".into(),
            kind: None,
        }],
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(run_id, &analysis).expect("insert analysis");

    // Ensure the expected run directory exists so the CLI can render paths.
    std::fs::create_dir_all(layout.binary_output_root("BinA").join("RunOne")).unwrap();

    // Invoke CLI and assert JSON includes persisted analysis counts.
    let output = cargo_bin_cmd!("binary-slicer")
        .arg("show-ritual-run")
        .arg("--root")
        .arg(&root)
        .arg("--binary")
        .arg("BinA")
        .arg("--ritual")
        .arg("RunOne")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let payload: Value =
        serde_json::from_slice(&output).expect("show-ritual-run output should be JSON");
    let meta = payload.get("metadata").expect("metadata section missing");
    assert_eq!(meta["backend_path"], "/usr/bin/rizin");
    let analysis_json = payload.get("analysis").expect("analysis section missing");
    assert_eq!(analysis_json["functions"].as_array().unwrap().len(), 1);
    assert_eq!(analysis_json["call_edges"].as_array().unwrap().len(), 1);
    assert_eq!(analysis_json["basic_blocks"].as_array().unwrap().len(), 1);
    assert_eq!(analysis_json["evidence"].as_array().unwrap().len(), 1);
}

#[test]
fn project_info_json_includes_analysis_summary() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // Init project and binary.
    init_project_command(root.to_str().unwrap(), Some("ProjA".into())).unwrap();
    let bin_path = root.join("binA.so");
    std::fs::write(&bin_path, b"payload").unwrap();
    cargo_bin_cmd!("binary-slicer")
        .arg("add-binary")
        .arg("--root")
        .arg(root)
        .arg("--path")
        .arg(&bin_path)
        .arg("--name")
        .arg("BinA")
        .assert()
        .success();

    let layout = ProjectLayout::new(root);
    let db = ProjectDb::open(&layout.db_path).expect("open db");
    let run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "RunInfo".into(),
        spec_hash: "specA".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: RitualRunStatus::Succeeded,
        started_at: "t0".into(),
        finished_at: "t1".into(),
    };
    let run_id = db.insert_ritual_run(&run).expect("insert run");
    let analysis = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x2000,
            name: Some("RunInfoFunc".into()),
            size: Some(12),
            in_slice: false,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x2000, to: 0x3000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x2000,
            len: 12,
            successors: vec![BlockEdge { target: 0x3000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord {
            address: 0x2000,
            description: "evidence".into(),
            kind: None,
        }],
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(run_id, &analysis).expect("insert analysis");

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

    let payload: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let runs = payload["ritual_runs"].as_array().unwrap();
    let summary = runs
        .iter()
        .find(|r| r["name"] == "RunInfo")
        .and_then(|r| r.get("analysis"))
        .expect("analysis summary missing");
    assert_eq!(summary["functions"].as_u64().unwrap(), 1);
    assert_eq!(summary["call_edges"].as_u64().unwrap(), 1);
    assert_eq!(summary["basic_blocks"].as_u64().unwrap(), 1);
    assert_eq!(summary["evidence"].as_u64().unwrap(), 1);
    assert_eq!(summary["backend_path"], "/usr/bin/rizin");
}

#[test]
fn list_ritual_runs_json_includes_analysis_summary() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    init_project_command(root.to_str().unwrap(), Some("ListAnalysis".into())).unwrap();
    let layout = ProjectLayout::new(root);
    let db = ProjectDb::open(&layout.db_path).unwrap();
    let run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "RunList".into(),
        spec_hash: "sh".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: RitualRunStatus::Succeeded,
        started_at: "t0".into(),
        finished_at: "t1".into(),
    };
    let run_id = db.insert_ritual_run(&run).unwrap();
    let analysis = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x3000,
            name: Some("ListFunc".into()),
            size: Some(8),
            in_slice: false,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x3000, to: 0x4000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x3000,
            len: 8,
            successors: vec![BlockEdge { target: 0x4000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord { address: 0x3000, description: "list".into(), kind: None }],
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(run_id, &analysis).unwrap();

    std::fs::create_dir_all(layout.binary_output_root("BinA").join("RunList")).unwrap();

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
    let runs: Value = serde_json::from_slice(&output).unwrap();
    let analysis_json = runs[0].get("analysis").expect("analysis missing");
    assert_eq!(analysis_json["functions"].as_u64().unwrap(), 1);
    assert_eq!(analysis_json["call_edges"].as_u64().unwrap(), 1);
    assert_eq!(analysis_json["backend_path"], "/usr/bin/rizin");
}
