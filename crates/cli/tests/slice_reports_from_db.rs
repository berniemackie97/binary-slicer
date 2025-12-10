use binary_slicer::commands::{
    emit_slice_docs_command, emit_slice_reports_command, init_project_command, init_slice_command,
};
use ritual_core::db::{ProjectDb, ProjectLayout, RitualRunRecord, RitualRunStatus};
use ritual_core::services::analysis::{
    AnalysisResult, BasicBlock, BlockEdge, BlockEdgeKind, CallEdge, EvidenceKind, EvidenceRecord,
    FunctionRecord,
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
        evidence: vec![EvidenceRecord {
            address: 0x1000,
            description: "import foo".into(),
            kind: Some(EvidenceKind::Import),
        }],
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
        evidence: vec![EvidenceRecord {
            address: 0x2000,
            description: "newer import".into(),
            kind: Some(EvidenceKind::Import),
        }],
        backend_version: Some("rz-2.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(newer_run_id, &newer_analysis).expect("insert newer analysis");

    // A third run on a different binary to exercise override behavior.
    let bin_b_run = RitualRunRecord {
        binary: "BinB".into(),
        ritual: "SliceOne".into(),
        spec_hash: "sh3".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: Some("rz-2.1".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: RitualRunStatus::Succeeded,
        started_at: "t10".into(),
        finished_at: "t11".into(),
    };
    let bin_b_run_id = db.insert_ritual_run(&bin_b_run).expect("insert bin b run");
    let bin_b_analysis = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x4000,
            name: Some("BinBFunc".into()),
            size: Some(10),
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x4000, to: 0x5000, is_cross_slice: false }],
        basic_blocks: vec![BasicBlock {
            start: 0x4000,
            len: 10,
            successors: vec![BlockEdge { target: 0x5000, kind: BlockEdgeKind::Jump }],
        }],
        evidence: vec![EvidenceRecord {
            address: 0x4000,
            description: "binb call".into(),
            kind: Some(EvidenceKind::Call),
        }],
        backend_version: Some("rz-2.1".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };
    db.insert_analysis_result(bin_b_run_id, &bin_b_analysis).expect("insert bin b analysis");

    emit_slice_reports_command(&root, None).unwrap();

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
    let ev = report["evidence"].as_array().unwrap();
    assert!(!ev.is_empty());
    assert!(ev.iter().any(|e| {
        e["kind"] == "import"
            || e["description"].as_str().unwrap_or_default().to_lowercase().contains("import")
    }));
    assert!(!report["call_edges"].as_array().unwrap().is_empty());
    let graph = std::fs::read_to_string(&graph_path).unwrap();
    assert!(graph.contains("NewerFunc") || graph.contains("0x2000"));
    assert!(graph.contains("call") || graph.contains("jump"));

    // Override binary to pick BinB run instead of default slice linkage.
    emit_slice_reports_command(&root, Some("BinB")).unwrap();
    let report_override: Value =
        serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
    let funcs_override = report_override["functions"].as_array().unwrap();
    assert_eq!(funcs_override[0]["name"].as_str().unwrap(), "BinBFunc");

    // Docs should include evidence/functions from the chosen run.
    emit_slice_docs_command(&root).unwrap();
    let doc_body = std::fs::read_to_string(layout.slices_docs_dir.join("SliceOne.md")).unwrap();
    assert!(doc_body.contains("BinA")); // default binary printed
    assert!(doc_body.contains("NewerFunc")); // uses default binary (BinA) run by default
    assert!(doc_body.contains("Evidence")); // evidence section populated
}
