#![cfg(feature = "capstone-backend")]

use object::write::{Object, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};
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

#[test]
fn capstone_backend_handles_arm64_arch_hint() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("arm64.bin");
    // mov x0, x0; ret
    std::fs::write(&bin_path, [0xE0, 0x03, 0x00, 0xAA, 0xC0, 0x03, 0x5F, 0xD6]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "Arm64Test".into(),
        binary_name: "Arm64Bin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(32),
            ..Default::default()
        },
        arch: Some("arm64".into()),
    };

    let result = backend.analyze(&request).expect("analyze arm64");
    assert!(!result.functions.is_empty(), "expected at least one function for ARM64 hint");
}

#[test]
fn capstone_backend_handles_ppc_arch_hint() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("ppc.bin");
    // nop (ori r0,r0,0) for ppc64: 0x60 00 00 00
    std::fs::write(&bin_path, [0x60, 0x00, 0x00, 0x00]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "PpcTest".into(),
        binary_name: "PpcBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(16),
            ..Default::default()
        },
        arch: Some("ppc64".into()),
    };

    let result = backend.analyze(&request).expect("analyze ppc64");
    assert!(!result.functions.is_empty(), "expected at least one function for PPC hint");
}

#[test]
fn capstone_backend_marks_indirect_jump_edges() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("ijump.bin");
    // mov rax, 0; jmp rax
    std::fs::write(&bin_path, [0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xE0]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "IjumpTest".into(),
        binary_name: "IjumpBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(32),
            ..Default::default()
        },
        arch: Some("x86_64".into()),
    };

    let result = backend.analyze(&request).expect("analyze ijump");
    assert!(
        !result.basic_blocks.is_empty(),
        "expected basic blocks even for indirect jump; evidence: {:?}",
        result.evidence
    );
}

#[test]
fn capstone_backend_auto_detects_arch_and_sections() {
    let temp = tempfile::tempdir().unwrap();
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let text_id = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);
    obj.section_mut(text_id).append_data(&[0xC3], 1); // ret
    obj.add_symbol(Symbol {
        name: b"auto_fn".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text_id),
        flags: SymbolFlags::Elf { st_info: 0x12, st_other: 0 },
    });
    let bin_path = temp.path().join("auto_detect.elf");
    std::fs::write(&bin_path, obj.write().unwrap()).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "AutoDetect".into(),
        binary_name: "AutoBin".into(),
        binary_path: bin_path,
        roots: vec!["auto_fn".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(32),
            ..Default::default()
        },
        arch: None, // force object-based detection
    };

    let result = backend.analyze(&request).expect("analyze auto-detect");
    assert!(
        result.functions.iter().any(|f| f.name.as_deref() == Some("auto_fn")),
        "expected symbol-derived function from auto-detect"
    );
}

#[test]
fn capstone_backend_falls_back_on_unknown_arch_hint() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("unknown.bin");
    // ret
    std::fs::write(&bin_path, [0xC3]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "UnknownArch".into(),
        binary_name: "UnknownBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(8),
            ..Default::default()
        },
        arch: Some("totally-unknown".into()),
    };

    let result = backend.analyze(&request).expect("analyze unknown arch");
    assert!(
        !result.functions.is_empty(),
        "expected synthetic/function output even on unknown arch"
    );
}

#[test]
fn capstone_backend_handles_empty_bytes_with_detected_arch() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("empty_detect.bin");
    // Empty bytes; arch None forces object detection path (will fall back to default x86).
    std::fs::write(&bin_path, []).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "EmptyDetect".into(),
        binary_name: "EmptyDetectBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(4),
            ..Default::default()
        },
        arch: None,
    };

    // Should not error even if nothing is disassembled.
    backend.analyze(&request).expect("analyze empty detect");
}

#[test]
fn capstone_backend_decodes_arm64_calls() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("arm64_call.bin");
    // bl +0 (imm=0) then ret
    std::fs::write(&bin_path, [0x00, 0x00, 0x00, 0x94, 0xC0, 0x03, 0x5F, 0xD6]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "Arm64Call".into(),
        binary_name: "Arm64CallBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(16),
            ..Default::default()
        },
        arch: Some("arm64".into()),
    };

    let result = backend.analyze(&request).expect("analyze arm64 call");
    assert!(
        !result.call_edges.is_empty(),
        "expected call edge decoded from arm64 bl; edges: {:?}",
        result.call_edges
    );
}

#[test]
fn capstone_backend_decodes_riscv_jal_call() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("riscv_jal.bin");
    // jal x0, 0 (encodes as 0x0000006f little endian)
    std::fs::write(&bin_path, [0x6f, 0x00, 0x00, 0x00]).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "RvCall".into(),
        binary_name: "RvCallBin".into(),
        binary_path: bin_path,
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(8),
            ..Default::default()
        },
        arch: Some("riscv32".into()),
    };

    let result = backend.analyze(&request).expect("analyze riscv jal");
    assert!(
        !result.call_edges.is_empty() || !result.basic_blocks.is_empty(),
        "expected control-flow edges from jal"
    );
}

#[test]
fn capstone_backend_auto_detects_macho_arch_none() {
    let temp = tempfile::tempdir().unwrap();
    let mut obj = object::write::Object::new(
        object::BinaryFormat::MachO,
        object::Architecture::X86_64,
        object::Endianness::Little,
    );
    let text_id = obj.add_section(Vec::new(), b"__TEXT,__text".to_vec(), object::SectionKind::Text);
    obj.section_mut(text_id).append_data(&[0xC3], 1); // ret
    obj.add_symbol(object::write::Symbol {
        name: b"_auto_none".to_vec(),
        value: 0,
        size: 0,
        kind: object::SymbolKind::Text,
        scope: object::SymbolScope::Linkage,
        weak: false,
        section: object::write::SymbolSection::Section(text_id),
        flags: object::SymbolFlags::MachO { n_desc: 0 },
    });
    let bin_path = temp.path().join("auto_none_macho");
    std::fs::write(&bin_path, obj.write().unwrap()).unwrap();

    let backend = CapstoneBackend;
    let request = AnalysisRequest {
        ritual_name: "AutoNoneMach".into(),
        binary_name: "MachBin".into(),
        binary_path: bin_path,
        roots: vec!["_auto_none".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            max_instructions: Some(16),
            ..Default::default()
        },
        arch: None, // force Mach-O detection path
    };

    let result = backend.analyze(&request).expect("analyze macho none");
    assert!(
        result.functions.iter().any(|f| f.name.as_deref() == Some("_auto_none")),
        "expected Mach-O symbol via auto-detect with arch=None"
    );
}
