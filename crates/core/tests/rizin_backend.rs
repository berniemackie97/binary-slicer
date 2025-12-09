#![cfg(feature = "rizin-backend")]

use ritual_core::services::analysis::{AnalysisBackend, AnalysisRequest, AnalysisResult};
use ritual_core::services::backends::RizinBackend;

#[test]
fn rizin_backend_errors_for_missing_binary() {
    let backend = RizinBackend;
    let req = AnalysisRequest {
        ritual_name: "Missing".into(),
        binary_name: "MissingBin".into(),
        binary_path: std::path::PathBuf::from("does_not_exist.bin"),
        roots: vec!["entry".into()],
        options: Default::default(),
        arch: None,
    };
    let err = backend.analyze(&req).unwrap_err();
    assert!(format!("{err:?}").contains("MissingBinary"));
}

#[test]
fn rizin_backend_parses_fake_json_without_rizin_installed() {
    let backend = RizinBackend;
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    std::fs::write(&bin, b"bin").unwrap();

    // Fake rizin output and version to avoid external dependency in CI.
    let fake_json = temp.path().join("aflj.json");
    std::fs::write(
        &fake_json,
        r#"[{"offset":4096,"name":"func_a","size":16,"callrefs":[{"addr":12288,"type":"C"}]},{"offset":8192,"name":"func_b","size":8}]"#,
    )
    .unwrap();
    let fake_graph = temp.path().join("agfj.json");
    std::fs::write(
        &fake_graph,
        r#"[{"offset":4096,"blocks":[{"offset":4096,"size":4,"jump":4100,"fail":4104}]}]"#,
    )
    .unwrap();
    let fake_strings = temp.path().join("strings.json");
    std::fs::write(&fake_strings, r#"[{"vaddr":8192,"string":"hello"}]"#).unwrap();
    std::env::set_var("BS_RIZIN_FAKE_JSON", &fake_json);
    std::env::set_var("BS_RIZIN_FAKE_VERSION", "rizin 1.0-fake");
    std::env::set_var("BS_RIZIN_FAKE_GRAPH", &fake_graph);
    std::env::set_var("BS_RIZIN_FAKE_STRINGS", &fake_strings);

    let req = AnalysisRequest {
        ritual_name: "Fake".into(),
        binary_name: "FakeBin".into(),
        binary_path: bin,
        roots: vec![],
        options: Default::default(),
        arch: None,
    };
    let result: AnalysisResult = backend.analyze(&req).expect("analyze fake");
    assert_eq!(result.functions.len(), 2);
    assert_eq!(result.functions[0].name.as_deref(), Some("func_a"));
    assert_eq!(result.backend_version.as_deref(), Some("rizin 1.0-fake"));
    assert_eq!(result.call_edges.len(), 1);
    assert_eq!(result.call_edges[0].from, 4096);
    assert_eq!(result.call_edges[0].to, 12288);
    assert!(!result.basic_blocks.is_empty());
    assert!(!result.evidence.is_empty());

    std::env::remove_var("BS_RIZIN_FAKE_JSON");
    std::env::remove_var("BS_RIZIN_FAKE_VERSION");
    std::env::remove_var("BS_RIZIN_FAKE_GRAPH");
    std::env::remove_var("BS_RIZIN_FAKE_STRINGS");
}
