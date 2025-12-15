use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::services::analysis::{
    AnalysisBackend, AnalysisError, AnalysisRequest, AnalysisResult, CallEdge, EvidenceRecord,
    FunctionRecord,
};

/// Resolve the analyzeHeadless executable path from environment variables.
///
/// Precedence:
/// - `GHIDRA_ANALYZE_HEADLESS` pointing directly to the executable.
/// - `GHIDRA_INSTALL_DIR`, appended with platform-specific analyzeHeadless name.
fn resolve_headless_path() -> Result<PathBuf, String> {
    if let Ok(p) = env::var("GHIDRA_ANALYZE_HEADLESS") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Ok(dir) = env::var("GHIDRA_INSTALL_DIR") {
        let mut p = PathBuf::from(dir);
        if cfg!(windows) {
            p = p.join("analyzeHeadless.bat");
        } else {
            p = p.join("analyzeHeadless");
        }
        if p.is_file() {
            return Ok(p);
        }
    }

    Err("Set GHIDRA_ANALYZE_HEADLESS (path to analyzeHeadless) or GHIDRA_INSTALL_DIR".to_string())
}

fn ghidra_version(headless: &Path) -> Result<String, String> {
    let output = Command::new(headless)
        .arg("-version")
        .output()
        .map_err(|e| format!("failed to spawn analyzeHeadless: {e}"))?;
    if !output.status.success() {
        return Err(format!("analyzeHeadless exited with {}", output.status));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        Err("analyzeHeadless returned empty version string".to_string())
    } else {
        Ok(first.to_string())
    }
}

/// Minimal Ghidra headless backend stub: validates binary, resolves headless path, and records
/// Ghidra version as evidence. Full analysis to follow in later iterations.
pub struct GhidraBackend;

impl AnalysisBackend for GhidraBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        if !request.binary_path.is_file() {
            return Err(AnalysisError::MissingBinary(request.binary_path.clone()));
        }

        let headless = resolve_headless_path().map_err(AnalysisError::Backend)?;
        let version = ghidra_version(&headless).map_err(AnalysisError::Backend)?;

        // Synthetic functions per root for now; real analysis will populate from Ghidra output.
        let functions: Vec<FunctionRecord> = request
            .roots
            .iter()
            .enumerate()
            .map(|(idx, name)| FunctionRecord {
                address: 0x3000 + idx as u64,
                name: Some(name.clone()),
                size: None,
                in_slice: true,
                is_boundary: false,
            })
            .collect();

        let evidence =
            vec![EvidenceRecord { address: 0, description: version.clone(), kind: None }];

        Ok(AnalysisResult {
            functions,
            call_edges: vec![CallEdge { from: 0, to: 0, is_cross_slice: false }],
            evidence,
            basic_blocks: vec![],
            roots: request.roots.clone(),
            root_hits: crate::services::analysis::build_root_hits(&request.roots, &functions),
            backend_version: Some(version),
            backend_path: Some(headless.to_string_lossy().to_string()),
        })
    }

    fn name(&self) -> &'static str {
        "ghidra"
    }
}
