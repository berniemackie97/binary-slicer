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
            kind: None,
        }],
        basic_blocks: vec![BasicBlock {
            start: 0x1000,
            len: 4,
            successors: vec![BlockEdge { target: 0x1004, kind: BlockEdgeKind::Fallthrough }],
        }],
        roots: vec!["root_a".into(), "root_b".into()],
        root_hits: vec![
            ritual_core::services::analysis::RootHit {
                root: "root_a".into(),
                functions: vec![0x1000],
            },
            ritual_core::services::analysis::RootHit {
                root: "root_b".into(),
                functions: Vec::new(),
            },
        ],
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
    assert_eq!(loaded.roots, vec!["root_a".to_string(), "root_b".to_string()]);

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

    let roots_count: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM analysis_roots WHERE run_id = ?", [run_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(roots_count, 2);
}

#[test]
fn insert_analysis_result_overwrites_existing_rows() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("proj.db");
    let db = ProjectDb::open(&db_path).unwrap();

    let run_record = ritual_core::db::RitualRunRecord {
        binary: "Bin".into(),
        ritual: "Run".into(),
        spec_hash: "spec".into(),
        binary_hash: None,
        backend: "rizin".into(),
        backend_version: None,
        backend_path: None,
        status: ritual_core::db::RitualRunStatus::Succeeded,
        started_at: "now".into(),
        finished_at: "now".into(),
    };
    let run_id = db.insert_ritual_run(&run_record).unwrap();

    let first = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x1,
            name: Some("f1".into()),
            size: None,
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x1, to: 0x2, is_cross_slice: false }],
        evidence: vec![ritual_core::services::analysis::EvidenceRecord {
            address: 0x1,
            description: "first".into(),
            kind: None,
        }],
        basic_blocks: vec![BasicBlock {
            start: 0x1,
            len: 4,
            successors: vec![BlockEdge { target: 0x2, kind: BlockEdgeKind::Jump }],
        }],
        roots: vec!["root1".into()],
        root_hits: vec![ritual_core::services::analysis::RootHit {
            root: "root1".into(),
            functions: vec![0x1],
        }],
        backend_version: None,
        backend_path: None,
    };
    db.insert_analysis_result(run_id, &first).unwrap();

    let second = AnalysisResult {
        functions: vec![FunctionRecord {
            address: 0x10,
            name: Some("f2".into()),
            size: None,
            in_slice: true,
            is_boundary: false,
        }],
        call_edges: vec![CallEdge { from: 0x10, to: 0x20, is_cross_slice: false }],
        evidence: vec![ritual_core::services::analysis::EvidenceRecord {
            address: 0x10,
            description: "second".into(),
            kind: None,
        }],
        basic_blocks: vec![BasicBlock {
            start: 0x10,
            len: 8,
            successors: vec![BlockEdge { target: 0x20, kind: BlockEdgeKind::Fallthrough }],
        }],
        roots: vec!["root2".into(), "root3".into()],
        root_hits: vec![
            ritual_core::services::analysis::RootHit {
                root: "root2".into(),
                functions: vec![0x10],
            },
            ritual_core::services::analysis::RootHit {
                root: "root3".into(),
                functions: Vec::new(),
            },
        ],
        backend_version: None,
        backend_path: None,
    };
    db.insert_analysis_result(run_id, &second).unwrap();

    let loaded = db.load_analysis_result("Bin", "Run").unwrap().expect("analysis result");
    assert_eq!(loaded.functions.len(), 1);
    assert_eq!(loaded.functions[0].address, 0x10);
    assert_eq!(loaded.call_edges.len(), 1);
    assert_eq!(loaded.evidence.len(), 1);
    assert_eq!(loaded.basic_blocks.len(), 1);
    assert_eq!(loaded.roots, vec!["root2".to_string(), "root3".to_string()]);
}
