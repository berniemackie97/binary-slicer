#![cfg(feature = "capstone-backend")]

use goblin::Object;
use object::write::{Object as ObjectWriter, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};
use ritual_core::services::analysis::{AnalysisBackend, AnalysisOptions, AnalysisRequest};
use ritual_core::services::backends::CapstoneBackend;
use std::path::PathBuf;

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
    let status = std::process::Command::new("rustc")
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
    let mut obj = ObjectWriter::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);

    // .text with one instruction stream.
    let text_id = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);

    // .rodata with a small string; we'll emit an immediate pointing here.
    let ro_id = obj.add_section(Vec::new(), b".rodata".to_vec(), SectionKind::ReadOnlyData);
    obj.section_mut(ro_id).append_data(b"hello\x00", 1);

    // Instructions:
    //  mov rax, imm64 (patched to .rodata start)
    //  mov rbx, rax (reg operand evidence)
    //  mov rax, [rbx+0x10] (mem operand evidence)
    //  ret
    let mut text = Vec::new();
    text.extend([0x48u8, 0xB8]); // mov rax, imm64
    text.extend(0u64.to_le_bytes());
    text.extend([0x48, 0x89, 0xC3]); // mov rbx, rax
    text.extend([0x48, 0x8B, 0x43, 0x10]); // mov rax, [rbx+0x10]
    text.push(0xC3); // ret
    obj.section_mut(text_id).set_data(text, 1);

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

    let mut bytes = obj.write().unwrap();
    let (ro_start, text_offset) = match Object::parse(&bytes).expect("parse object for operands") {
        Object::Elf(elf) => {
            let names: Vec<_> =
                elf.section_headers.iter().filter_map(|sh| elf.strtab.get_at(sh.sh_name)).collect();
            let ro = elf.section_headers.iter().find(|sh| {
                let name = elf.strtab.get_at(sh.sh_name).unwrap_or("");
                name == ".rodata"
                    || (sh.sh_size > 0
                        && sh.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR) == 0
                        && name.contains("rodata"))
            });
            let ro = ro
                .or_else(|| {
                    elf.section_headers.iter().find(|sh| {
                        sh.sh_size > 0
                            && sh.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR)
                                == 0
                    })
                })
                .unwrap_or_else(|| panic!("rodata header; saw sections {names:?}"));
            let text = elf.section_headers.iter().find(|sh| {
                elf.strtab.get_at(sh.sh_name) == Some(".text")
                    || sh.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR) != 0
            });
            let text = text
                .or_else(|| {
                    elf.section_headers.iter().find(|sh| {
                        sh.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR) != 0
                    })
                })
                .unwrap_or_else(|| panic!("text header; saw sections {names:?}"));
            (ro.sh_addr, text.sh_offset as usize)
        }
        other => panic!("unexpected object {:?}", other),
    };
    let imm_offset = text_offset + 2; // after opcode
    bytes[imm_offset..imm_offset + 8].copy_from_slice(&ro_start.to_le_bytes());

    let bin_path = temp.path().join("fixture_elf");
    std::fs::write(&bin_path, bytes).unwrap();

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
    let mut obj = ObjectWriter::new(BinaryFormat::MachO, Architecture::X86_64, Endianness::Little);

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

#[test]
fn capstone_auto_detects_pe_arch_and_symbols() {
    // Windows-only fixture builder; skip elsewhere.
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("skipping PE auto-detect on non-Windows");
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let bin_path = build_pe_fixture();
        let backend = CapstoneBackend;
        let request = AnalysisRequest {
            ritual_name: "AutoPE".into(),
            binary_name: "AutoPEBin".into(),
            binary_path: bin_path,
            roots: vec!["add".into()],
            options: AnalysisOptions {
                max_depth: Some(1),
                max_instructions: Some(64),
                ..Default::default()
            },
            arch: None, // force PE arch detection
        };
        let result = backend.analyze(&request).expect("analyze pe auto");
        assert!(
            result.functions.iter().any(|f| f.name.as_deref() == Some("add")),
            "expected exported symbol detected from PE"
        );
    }
}

#[test]
fn capstone_auto_detects_macho_arch() {
    let temp = tempfile::tempdir().unwrap();
    let mut obj = ObjectWriter::new(BinaryFormat::MachO, Architecture::X86_64, Endianness::Little);
    let text_id = obj.add_section(Vec::new(), b"__TEXT,__text".to_vec(), SectionKind::Text);
    obj.section_mut(text_id).append_data(&[0xC3], 1);
    obj.add_symbol(Symbol {
        name: b"_auto_mach".to_vec(),
        value: 0,
        size: 0,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text_id),
        flags: SymbolFlags::MachO { n_desc: 0 },
    });
    let bin_path = temp.path().join("auto_mach");
    std::fs::write(&bin_path, obj.write().unwrap()).unwrap();

    let backend = CapstoneBackend;
    let request = capstone_request(bin_path, vec!["_auto_mach".into()]);
    let result = backend.analyze(&request).expect("analyze macho auto");
    assert!(
        result.functions.iter().any(|f| f.name.as_deref() == Some("_auto_mach")),
        "expected Mach-O symbol via auto-detect"
    );
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
