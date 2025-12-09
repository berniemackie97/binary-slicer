use binary_slicer::commands::{
    emit_slice_reports_command, init_project_command, init_slice_command,
};
use ritual_core::db::{ProjectDb, ProjectLayout, RitualRunRecord, RitualRunStatus};
use ritual_core::services::analysis::{
    AnalysisResult, BasicBlock, BlockEdge, BlockEdgeKind, CallEdge, EvidenceRecord, FunctionRecord,
};
use serde_json::Value;
use tempfile::tempdir;

#[test]
fn emit_slice_reports_and_graphs_use_db_analysis() {
    let temp = tempdir().unwrap();
    let root = temp.path().to_string_lossy().to_string();

    init_project_command(&root, Some("SliceProj".into())).unwrap();
    init_slice_command(&root, "SliceOne", Some("demo slice".into()), Some("BinA".into())).unwrap();

    // Seed a ritual run + analysis with the same name as the slice.
    let layout = ProjectLayout::new(&root);
    let db = ProjectDb::open(&layout.db_path).expect("open db");
    let run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "SliceOne".into(),
        spec_hash: "sh".into(),
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
            address: 0x1000,
            name: Some("SliceFunc".into()),
            size: Some(8),
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x1000, to: 0x2000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x1000,
            len: 8,
            successors: vec![BlockEdge { target: 0x2000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord { address: 0x1000, description: "evidence".into() }],
        backend_version: Some("rz-1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(run_id, &analysis).expect("insert analysis");

    // Insert a newer run to verify we pick the freshest data.
    let newer_run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "SliceOne".into(),
        spec_hash: "sh2".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: Some("rz-2.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: RitualRunStatus::Succeeded,
        started_at: "t2".into(),
        finished_at: "t9".into(),
    };
    let newer_run_id = db.insert_ritual_run(&newer_run).expect("insert newer run");
    let newer_analysis = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x2000,
            name: Some("NewerFunc".into()),
            size: Some(12),
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x2000, to: 0x3000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x2000,
            len: 12,
            successors: vec![BlockEdge { target: 0x3000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord { address: 0x2000, description: "newer".into() }],
        backend_version: Some("rz-2.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(newer_run_id, &newer_analysis).expect("insert newer analysis");

    emit_slice_reports_command(&root).unwrap();

    let report_path = layout.reports_dir.join("SliceOne.json");
    let graph_path = layout.graphs_dir.join("SliceOne.dot");
    assert!(report_path.is_file(), "report should be written");
    assert!(graph_path.is_file(), "graph should be written");

    let report: Value =
        serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
    let funcs = report["functions"].as_array().unwrap();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0]["name"].as_str().unwrap(), "NewerFunc");
    assert_eq!(report["call_edges"].as_array().unwrap().len(), 1);
    let graph = std::fs::read_to_string(&graph_path).unwrap();
    assert!(graph.contains("NewerFunc") || graph.contains("0x2000"));
    assert!(graph.contains("call") || graph.contains("jump"));
}
