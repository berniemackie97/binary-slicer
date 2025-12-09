use ritual_core::db::ProjectDb;
use ritual_core::services::analysis::{
    AnalysisResult, BasicBlock, BlockEdge, BlockEdgeKind, CallEdge, FunctionRecord,
};

#[test]
fn analysis_result_is_persisted_with_run() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("proj.db");
    let db = ProjectDb::open(&db_path).unwrap();

    let run_record = ritual_core::db::RitualRunRecord {
        binary: "Bin".into(),
        ritual: "Run".into(),
        spec_hash: "spec".into(),
        binary_hash: Some("binhash".into()),
        backend: "rizin".into(),
        backend_version: Some("1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
        status: ritual_core::db::RitualRunStatus::Succeeded,
        started_at: "now".into(),
        finished_at: "now".into(),
    };
    let run_id = db.insert_ritual_run(&run_record).unwrap();

    let result = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x1000,
            name: Some("func".into()),
            size: Some(8),
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x1000, to: 0x2000, is_cross_slice: false }],
        evidence: vec![ritual_core::services::analysis::EvidenceRecord {
            address: 0x1000,
            description: "string: test".into(),
        }],
        basic_blocks: vec![BasicBlock {
            start: 0x1000,
            len: 4,
            successors: vec![BlockEdge { target: 0x1004, kind: BlockEdgeKind::Fallthrough }],
        }],
        backend_version: Some("1.0".into()),
        backend_path: Some("/usr/bin/rizin".into()),
    };

    db.insert_analysis_result(run_id, &result).unwrap();

    // Load back via helper.
    let loaded = db.load_analysis_result("Bin", "Run").unwrap().expect("analysis result");
    assert_eq!(loaded.functions.len(), 1);
    assert_eq!(loaded.call_edges.len(), 1);
    assert_eq!(loaded.basic_blocks.len(), 1);
    assert_eq!(loaded.evidence.len(), 1);

    // Spot-check that data was written.
    let func_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM analysis_functions WHERE run_id = ?", [run_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(func_count, 1);

    let edge_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM analysis_call_edges WHERE run_id = ?", [run_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(edge_count, 1);

    let bb_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM analysis_basic_blocks WHERE run_id = ?", [run_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(bb_count, 1);

    let evidence_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM analysis_evidence WHERE run_id = ?", [run_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(evidence_count, 1);
}
