use std::process::Command;

use crate::services::analysis::{
    AnalysisBackend, AnalysisError, AnalysisRequest, AnalysisResult, CallEdge, EvidenceRecord,
    FunctionRecord,
};

/// Minimal rizin-backed analyzer that validates the binary and captures `rizin -v`.
pub struct RizinBackend;

impl AnalysisBackend for RizinBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        if !request.binary_path.is_file() {
            return Err(AnalysisError::MissingBinary(request.binary_path.clone()));
        }

        // Capture rizin version as evidence; if rizin is missing, surface a backend error.
        let version = version_string().map_err(AnalysisError::Backend)?;

        let functions: Vec<FunctionRecord> = request
            .roots
            .iter()
            .enumerate()
            .map(|(idx, name)| FunctionRecord {
                address: 0x2000 + idx as u64,
                name: Some(name.clone()),
                size: None,
                in_slice: true,
                is_boundary: false,
            })
            .collect();

        let evidence = vec![EvidenceRecord { address: 0, description: version.clone() }];

        Ok(AnalysisResult {
            functions,
            call_edges: vec![CallEdge { from: 0, to: 0, is_cross_slice: false }],
            evidence,
            basic_blocks: vec![],
            backend_version: Some(version),
            backend_path: None,
        })
    }

    fn name(&self) -> &'static str {
        "rizin"
    }
}

fn version_string() -> Result<String, String> {
    let output = Command::new("rizin")
        .arg("-v")
        .output()
        .map_err(|e| format!("failed to spawn rizin: {e}"))?;
    if !output.status.success() {
        return Err(format!("rizin -v exited with {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        Err("rizin -v produced no output".to_string())
    } else {
        Ok(stdout)
    }
}
