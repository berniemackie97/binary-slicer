#![cfg(feature = "capstone-backend")]

use std::path::PathBuf;
use std::process::Command;

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
