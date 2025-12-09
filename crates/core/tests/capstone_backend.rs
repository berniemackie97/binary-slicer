#![cfg(feature = "capstone-backend")]

use ritual_core::services::analysis::{AnalysisBackend, AnalysisOptions, AnalysisRequest};
use ritual_core::services::backends::CapstoneBackend;

#[test]
fn capstone_backend_disassembles_and_returns_functions() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("sample.bin");
    // Simple x86_64 prologue/ret
    // push rbp; mov rbp, rsp; call +0; ret
    std::fs::write(&bin_path, [0x55, 0x48, 0x89, 0xE5, 0xE8, 0x00, 0x00, 0x00, 0x00, 0xC3])
        .unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "CapTest".into(),
        binary_name: "CapBin".into(),
        binary_path: bin_path.clone(),
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            include_imports: false,
            include_strings: false,
            max_instructions: Some(32),
        },
        arch: Some("x86_64".into()),
    };

    let result = backend.analyze(&request).expect("analyze");
    assert_eq!(result.functions.len(), 1);
    let fname = result.functions[0].name.as_deref().expect("function name");
    assert!(
        fname == "entry_point" || fname.starts_with("sub_"),
        "unexpected synthetic name: {fname}"
    );
    assert!(!result.evidence.is_empty(), "expected some disassembly evidence");
    assert!(
        result.evidence.iter().any(|e| e.description.contains("basic_block")),
        "expected basic block evidence"
    );
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks to be exported");
    assert!(!result.call_edges.is_empty(), "expected at least one call edge from disassembly");
}

#[test]
fn capstone_backend_emits_edges_and_block_kinds() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("flow.bin");
    // x86_64 bytes:
    // 0: push rbp
    // 1: call +0 (targets addr 6)
    // 6: je +2  -> addr 10
    // 8: ret
    // 9: nop
    // 10: ret (target of je)
    let bytes = [0x55, 0xE8, 0x00, 0x00, 0x00, 0x00, 0x74, 0x02, 0xC3, 0x90, 0xC3];
    std::fs::write(&bin_path, bytes).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "FlowTest".into(),
        binary_name: "FlowBin".into(),
        binary_path: bin_path.clone(),
        roots: vec!["entry_point".into()],
        options: AnalysisOptions { max_depth: Some(1), max_instructions: Some(64), ..Default::default() },
        arch: Some("x86_64".into()),
    };

    let result = backend.analyze(&request).expect("analyze");
    assert!(
        !result.call_edges.is_empty(),
        "expected at least one call edge from synthetic flow"
    );
    let has_call = result
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.successors.iter())
        .any(|s| matches!(s.kind, ritual_core::services::analysis::BlockEdgeKind::Call));
    let has_jump = result
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.successors.iter())
        .any(|s| matches!(s.kind, ritual_core::services::analysis::BlockEdgeKind::Jump | ritual_core::services::analysis::BlockEdgeKind::ConditionalJump | ritual_core::services::analysis::BlockEdgeKind::IndirectJump));
    assert!(has_jump, "expected a jump successor");
    assert!(has_call, "expected a call successor edge");
}
