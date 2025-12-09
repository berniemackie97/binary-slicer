use std::fs;
use std::path::PathBuf;

use capstone::prelude::*;

use crate::services::analysis::{
    AnalysisBackend, AnalysisError, AnalysisRequest, AnalysisResult, CallEdge, EvidenceRecord,
    FunctionRecord,
};

/// A minimal Capstone-backed analyzer.
///
/// For now we assume x86_64 mode and emit one synthetic function per root with
/// the root name as the function name, plus a small slice of disassembly as evidence.
pub struct CapstoneBackend;

impl CapstoneBackend {
    fn load_bytes(path: &PathBuf) -> Result<Vec<u8>, AnalysisError> {
        fs::read(path).map_err(|_| AnalysisError::MissingBinary(path.clone()))
    }
}

impl AnalysisBackend for CapstoneBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        let bytes = Self::load_bytes(&request.binary_path)?;
        if bytes.is_empty() {
            return Ok(AnalysisResult { functions: vec![], call_edges: vec![], evidence: vec![] });
        }

        // Try to disassemble a small window of the binary.
        let cs = Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode64)
            .build()
            .map_err(|e| AnalysisError::Backend(format!("capstone init failed: {e}")))?;

        let insns = cs
            .disasm_all(&bytes, 0)
            .map_err(|e| AnalysisError::Backend(format!("capstone disasm failed: {e}")))?;

        let mut evidence = Vec::new();
        for i in insns.iter().take(8) {
            evidence.push(EvidenceRecord {
                address: i.address(),
                description: format!("{} {}", i.mnemonic().unwrap_or(""), i.op_str().unwrap_or("")),
            });
        }

        // Emit one function per root, using a deterministic synthetic address per root index.
        let functions: Vec<FunctionRecord> = request
            .roots
            .iter()
            .enumerate()
            .map(|(idx, name)| FunctionRecord {
                address: 0x1000 + idx as u64,
                name: Some(name.clone()),
                size: None,
                in_slice: true,
                is_boundary: false,
            })
            .collect();

        Ok(AnalysisResult {
            functions,
            call_edges: vec![CallEdge { from: 0, to: 0, is_cross_slice: false }],
            evidence,
        })
    }

    fn name(&self) -> &'static str {
        "capstone"
    }
}
