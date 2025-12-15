use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{ProjectContext, RitualRunRecord, RitualRunStatus};

/// Minimal IR for functions encountered during analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRecord {
    pub address: u64,
    pub name: Option<String>,
    pub size: Option<u32>,
    pub in_slice: bool,
    pub is_boundary: bool,
}

/// Call edge between functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallEdge {
    pub from: u64,
    pub to: u64,
    pub is_cross_slice: bool,
}

/// Evidence to justify classification/decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub address: u64,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EvidenceKind>,
}

/// Optional classification for evidence entries to support grouping in reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    String,
    Import,
    Call,
    Other,
}

/// Kind of control-flow edge for a basic block successor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockEdgeKind {
    Fallthrough,
    Jump,
    ConditionalJump,
    IndirectJump,
    Call,
    IndirectCall,
}

/// Basic block representation for lightweight CFG export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicBlock {
    pub start: u64,
    pub len: u32,
    pub successors: Vec<BlockEdge>,
}

/// Successor edge with target and edge classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEdge {
    pub target: u64,
    pub kind: BlockEdgeKind,
}

/// Result of analyzing a ritual specification against a binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub functions: Vec<FunctionRecord>,
    pub call_edges: Vec<CallEdge>,
    pub evidence: Vec<EvidenceRecord>,
    pub basic_blocks: Vec<BasicBlock>,
    pub backend_version: Option<String>,
    pub backend_path: Option<String>,
}

/// Metadata to persist alongside an analysis run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunMetadata {
    pub spec_hash: String,
    pub binary_hash: Option<String>,
    pub backend: String,
    pub backend_version: Option<String>,
    pub backend_path: Option<String>,
    pub status: RitualRunStatus,
}

/// Options for analysis traversal.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AnalysisOptions {
    pub max_depth: Option<u32>,
    pub include_imports: bool,
    pub include_strings: bool,
    /// Optional instruction budget for backends that disassemble.
    pub max_instructions: Option<usize>,
}

/// Request to analyze a binary for a ritual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub ritual_name: String,
    pub binary_name: String,
    pub binary_path: PathBuf,
    pub roots: Vec<String>,
    /// Optional architecture hint (e.g., x86_64, arm64, armv7).
    pub arch: Option<String>,
    pub options: AnalysisOptions,
    /// Optional explicit backend tool path (e.g., configured rizin/ghidra path).
    pub backend_path: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("Binary not found at {0}")]
    MissingBinary(PathBuf),
    #[error("Backend not found: {0}")]
    MissingBackend(String),
    #[error("Analysis backend error: {0}")]
    Backend(String),
}

/// Trait implemented by analysis backends (e.g., Capstone + rizin).
pub trait AnalysisBackend: Send + Sync {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError>;
    fn name(&self) -> &'static str;
}

/// Registry for analysis backends; callers select by name.
#[derive(Default)]
pub struct BackendRegistry {
    backends: HashMap<String, Box<dyn AnalysisBackend>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self { backends: HashMap::new() }
    }

    pub fn register<B: AnalysisBackend + 'static>(&mut self, backend: B) -> &mut Self {
        self.backends.insert(backend.name().to_string(), Box::new(backend));
        self
    }

    pub fn get(&self, name: &str) -> Option<&dyn AnalysisBackend> {
        self.backends.get(name).map(|b| &**b)
    }

    /// Return a sorted list of registered backend names for error messages/help.
    pub fn names(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.backends.keys().cloned().collect();
        keys.sort();
        keys
    }
}

/// Coordinator that ties project context + backend to persist run results.
pub struct RitualRunner<'a> {
    pub ctx: &'a ProjectContext,
    pub backend: &'a dyn AnalysisBackend,
}

impl<'a> RitualRunner<'a> {
    pub fn run(
        &self,
        request: &AnalysisRequest,
        meta: &RunMetadata,
    ) -> Result<AnalysisResult, AnalysisError> {
        // Verify binary exists on disk if provided as a relative path in config.
        if !request.binary_path.is_file() {
            return Err(AnalysisError::MissingBinary(request.binary_path.clone()));
        }

        let mut result = self.backend.analyze(request)?;
        if result.backend_path.is_none() {
            result.backend_path = request.backend_path.as_ref().map(|p| p.display().to_string());
        }
        if result.backend_version.is_none() && meta.backend_version.is_some() {
            result.backend_version = meta.backend_version.clone();
        }

        // Persist a ritual run record in the DB (stub status until we store richer data).
        let now = Utc::now().to_rfc3339();
        let run_record = RitualRunRecord {
            binary: request.binary_name.clone(),
            ritual: request.ritual_name.clone(),
            spec_hash: meta.spec_hash.clone(),
            binary_hash: meta.binary_hash.clone(),
            backend: meta.backend.clone(),
            backend_version: result
                .backend_version
                .clone()
                .or_else(|| meta.backend_version.clone()),
            backend_path: result.backend_path.clone().or_else(|| meta.backend_path.clone()),
            status: meta.status.clone(),
            started_at: now.clone(),
            finished_at: now,
        };
        let run_id = self.ctx.db.insert_ritual_run(&run_record).ok();
        if let Some(id) = run_id {
            // Best-effort persistence of analysis details; ignore errors to avoid failing the run.
            let _ = self.ctx.db.insert_analysis_result(id, &result);
        }

        Ok(result)
    }
}

/// A minimal backend that validates the binary exists and produces empty results.
/// Useful until a real backend (Capstone/rizin) is configured.
pub struct ValidateOnlyBackend;

impl AnalysisBackend for ValidateOnlyBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        if !request.binary_path.is_file() {
            return Err(AnalysisError::MissingBinary(request.binary_path.clone()));
        }
        Ok(AnalysisResult {
            functions: vec![],
            call_edges: vec![],
            evidence: vec![],
            basic_blocks: vec![],
            backend_version: Some("validate-only".into()),
            backend_path: None,
        })
    }

    fn name(&self) -> &'static str {
        "validate-only"
    }
}

/// Convenience builder for a registry populated with the validate-only backend.
pub fn default_backend_registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(ValidateOnlyBackend);
    #[cfg(feature = "capstone-backend")]
    {
        registry.register(crate::services::backends::CapstoneBackend);
    }
    #[cfg(feature = "rizin-backend")]
    {
        registry.register(crate::services::backends::RizinBackend);
    }
    #[cfg(feature = "ghidra-backend")]
    {
        registry.register(crate::services::backends::GhidraBackend);
    }
    registry
}
