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
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(64),
            ..Default::default()
        },
        arch: Some("x86_64".into()),
    };

    let result = backend.analyze(&request).expect("analyze");
    assert!(!result.call_edges.is_empty(), "expected at least one call edge from synthetic flow");
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks to be produced from flow");
}

#[test]
fn capstone_backend_falls_back_to_roots_when_no_symbols_or_edges() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("empty.bin");
    std::fs::write(&bin_path, [0u8; 8]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "Empty".into(),
        binary_name: "EmptyBin".into(),
        binary_path: bin_path,
        roots: vec!["root_a".into(), "root_b".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(8),
            ..Default::default()
        },
        arch: Some("x86_64".into()),
    };

    let result = backend.analyze(&request).expect("analyze empty");
    assert_eq!(result.functions.len(), 2, "should fall back to roots as synthetic functions");
}

#[test]
fn capstone_backend_handles_arm_arch_hint() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("arm.bin");
    // mov r0, #0; bx lr
    std::fs::write(&bin_path, [0x00, 0x00, 0xA0, 0xE3, 0x1E, 0xFF, 0x2F, 0xE1]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "ArmTest".into(),
        binary_name: "ArmBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(32),
            ..Default::default()
        },
        arch: Some("arm".into()),
    };

    let result = backend.analyze(&request).expect("analyze arm");
    assert!(!result.functions.is_empty(), "expected at least one function for ARM hint");
}

#[test]
fn capstone_backend_handles_riscv32_arch_hint() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("riscv.bin");
    // addi x0, x0, 0
    std::fs::write(&bin_path, [0x13, 0x00, 0x00, 0x00]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "RvTest".into(),
        binary_name: "RvBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(32),
            ..Default::default()
        },
        arch: Some("riscv32".into()),
    };

    let result = backend.analyze(&request).expect("analyze riscv32");
    assert!(!result.functions.is_empty(), "expected at least one function for RISC-V hint");
}
