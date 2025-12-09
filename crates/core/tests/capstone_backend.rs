#![cfg(feature = "capstone-backend")]

use ritual_core::services::analysis::{AnalysisOptions, AnalysisRequest};
use ritual_core::services::backends::CapstoneBackend;

#[test]
fn capstone_backend_disassembles_and_returns_functions() {
    let temp = tempfile::tempdir().unwrap();
    let bin_path = temp.path().join("sample.bin");
    // Simple x86_64 prologue/ret
    std::fs::write(&bin_path, [0x55, 0x48, 0x89, 0xE5, 0xC3]).unwrap();

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
        },
    };

    let result = backend.analyze(&request).expect("analyze");
    assert_eq!(result.functions.len(), 1);
    assert_eq!(result.functions[0].name.as_deref(), Some("entry_point"));
    assert!(!result.evidence.is_empty(), "expected some disassembly evidence");
}
