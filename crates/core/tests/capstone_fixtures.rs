#![cfg(feature = "capstone-backend")]

use std::path::PathBuf;
use std::process::Command;

use object::write::{Objectection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};
use ritual_core::services::analysis::{AnalysisBackend, AnalysisOptions, AnalysisRequest};
use ritual_core::services::backends::CapstoneBackend;

fn capstone_request(binary_path: PathBuf, roots: Vec<String>) -> AnalysisRequest {
    AnalysisRequest {
        ritual_name: "FixtureTest".into(),
        binary_name: "FixtureBin".into(),
        binary_path,
        roots,
        options: AnalysisOptions {
            max_depth: Some(1),
            include_imports: false,
            include_strings: false,
            max_instructions: Some(256),
        },
        arch: Some("x86_64".into()),
    }
}

#[cfg(target_os = "windows")]
fn build_pe_fixture() -> PathBuf {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("lib.rs");
    std::fs::write(
        &src,
        r#"
        #[no_mangle]
        pub extern "C" fn add(a: i32, b: i32) -> i32 { a + b }
        "#,
    )
    .unwrap();
    let out = temp.path().join("fixture.dll");
    let status = Command::new("rustc")
        .args(["--crate-type=cdylib", src.to_str().unwrap(), "-o"])
        .arg(&out)
        .status()
        .expect("rustc spawn");
    assert!(status.success(), "rustc failed to build fixture dll");
    // Keep the directory alive for the duration of the test by leaking it.
    #[allow(deprecated)]
    let _ = temp.into_path();
    out
}

#[cfg(target_os = "windows")]
#[test]
fn capstone_handles_pe_with_exports() {
    let bin_path = build_pe_fixture();
    let backend = CapstoneBackend;
    let request = capstone_request(bin_path, vec!["add".into()]);
    let result = backend.analyze(&request).expect("analyze pe");
    assert!(!result.functions.is_empty(), "expected functions from PE export symbols");
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks from PE fixture");
}

#[test]
fn capstone_handles_minimal_elf_with_symbols_and_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    // .text with one instruction stream.
    let text_id = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);

    // .rodata with a small string; we'll emit an immediate pointing here.
    let ro_id = obj.add_section(Vec::new(), b".rodata".to_vec(), SectionKind::ReadOnlyData);
    obj.section_mut(ro_id).append_data(b"hello\x00", 1);

    // mov rax, imm64 (0x0) to trigger operand evidence against .rodata start.
    obj.section_mut(text_id)
        .set_data(Vec::from_iter([0x48u8, 0xB8].into_iter().chain(0u64.to_le_bytes())), 1);

    // Symbol for the function.
    obj.add_symbol(Symbol {
        name: b"hello_fn".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text_id),
        flags: SymbolFlags::Elf { st_info: 0x12, st_other: 0 },
    });

    let bin_path = temp.path().join("fixture_elf");
    std::fs::write(&bin_path, obj.write().unwrap()).unwrap();

    let backend = CapstoneBackend;
    let request = capstone_request(bin_path, vec!["hello_fn".into()]);
    let result = backend.analyze(&request).expect("analyze elf");
    assert!(
        result.functions.iter().any(|f| f.name.as_deref() == Some("hello_fn")),
        "expected symbol-derived function"
    );
    assert!(!result.evidence.is_empty(), "expected operand evidence from ELF fixture");
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks from ELF fixture");
}

#[test]
fn capstone_handles_minimal_macho_with_symbol() {
    let temp = tempfile::tempdir().unwrap();
    let mut obj = Object::new(BinaryFormat::MachO, Architecture::X86_64, Endianness::Little);

    // .text with ret
    let text_id = obj.add_section(Vec::new(), b"__TEXT,__text".to_vec(), SectionKind::Text);
    obj.section_mut(text_id).append_data(&[0xC3], 1);

    obj.add_symbol(Symbol {
        name: b"_mach_fn".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text_id),
        flags: SymbolFlags::MachO { n_desc: 0 },
    });

    let bin_path = temp.path().join("fixture_macho");
    std::fs::write(&bin_path, obj.write().unwrap()).unwrap();

    let backend = CapstoneBackend;
    let request = capstone_request(bin_path, vec!["_mach_fn".into()]);
    let result = backend.analyze(&request).expect("analyze macho");
    assert!(
        result.functions.iter().any(|f| f.name.as_deref() == Some("_mach_fn")),
        "expected Mach-O symbol-derived function"
    );
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks from Mach-O fixture");
}

#[cfg(target_os = "macos")]
#[test]
fn capstone_handles_macho_if_fixture_provided() {
    // To avoid embedding Mach-O from Windows CI, allow opt-in via env.
    let fixture = std::env::var("BS_MACHO_FIXTURE")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.is_file())
        .unwrap_or_else(|| {
            eprintln!("Skipping Mach-O fixture test; set BS_MACHO_FIXTURE to a valid path");
            std::process::exit(0)
        });
    let backend = CapstoneBackend;
    let request = capstone_request(fixture, vec!["main".into()]);
    let result = backend.analyze(&request).expect("analyze macho");
    assert!(!result.functions.is_empty(), "expected functions from Mach-O fixture");
    assert!(!result.basic_blocks.is_empty(), "expected basic blocks from Mach-O fixture");
}
