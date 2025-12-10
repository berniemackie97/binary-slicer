use ritual_core::db::ProjectContext;
use ritual_core::services::analysis::{
    AnalysisBackend, AnalysisOptions, AnalysisRequest, AnalysisResult, BackendRegistry,
    EvidenceRecord, FunctionRecord, RitualRunner, RunMetadata,
};

struct NoopBackend;

impl AnalysisBackend for NoopBackend {
    fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisResult, ritual_core::services::analysis::AnalysisError> {
        // Return an empty, deterministic result; caller is responsible for persistence.
        Ok(AnalysisResult {
            functions: request
                .roots
                .iter()
                .map(|r| FunctionRecord {
                    address: 0,
                    name: Some(r.clone()),
                    size: None,
                    in_slice: true,
                    is_boundary: false,
                })
                .collect(),
            call_edges: vec![],
            evidence: vec![EvidenceRecord {
                address: 0,
                description: "noop backend".into(),
                kind: None,
            }],
            basic_blocks: vec![],
            backend_version: Some("noop-1.0".into()),
            backend_path: None,
        })
    }

    fn name(&self) -> &'static str {
        "noop"
    }
}

#[test]
fn backend_registry_registers_and_resolves() {
    let mut registry = BackendRegistry::new();
    registry.register(NoopBackend);
    assert!(registry.get("noop").is_some());
}

#[test]
fn ritual_runner_invokes_backend_and_inserts_run() {
    let temp = tempfile::tempdir().unwrap();
    let layout = ritual_core::db::ProjectLayout::new(temp.path());
    std::fs::create_dir_all(&layout.meta_dir).unwrap();
    let config =
        ritual_core::db::ProjectConfig::new("CtxService", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();
    let ctx = ProjectContext::from_root(temp.path()).expect("ctx");
    let mut registry = BackendRegistry::new();
    registry.register(NoopBackend);
    let backend = registry.get("noop").unwrap();

    // Create a dummy binary file so the runner sees it on disk.
    let bin_path = temp.path().join("bin.so");
    std::fs::write(&bin_path, b"bin").unwrap();

    let request = AnalysisRequest {
        ritual_name: "TestRitual".into(),
        binary_name: "Bin".into(),
        binary_path: bin_path.clone(),
        roots: vec!["entry_point".into()],
        options: AnalysisOptions {
            max_depth: Some(1),
            include_imports: false,
            include_strings: false,
            max_instructions: Some(16),
        },
        arch: None,
    };

    let runner = RitualRunner { ctx: &ctx, backend };
    let result = runner
        .run(
            &request,
            &RunMetadata {
                spec_hash: "hash123".into(),
                binary_hash: Some("binhash".into()),
                backend: "noop".into(),
                backend_version: None,
                backend_path: None,
                status: ritual_core::db::RitualRunStatus::Succeeded,
            },
        )
        .expect("analysis");
    assert_eq!(result.functions.len(), 1);
    assert_eq!(result.functions[0].name.as_deref(), Some("entry_point"));

    // DB should contain a run record.
    let runs = ctx.db.list_ritual_runs(None).expect("runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, ritual_core::db::RitualRunStatus::Succeeded);
}
