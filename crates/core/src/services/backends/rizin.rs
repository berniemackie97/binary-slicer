use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::services::analysis::{
    AnalysisBackend, AnalysisError, AnalysisRequest, AnalysisResult, CallEdge, EvidenceRecord,
    FunctionRecord,
};

/// Rizin-backed analyzer that shells out to rizin/rz with a minimal script to gather symbols.
pub struct RizinBackend;

impl AnalysisBackend for RizinBackend {
    fn analyze(&self, request: &AnalysisRequest) -> Result<AnalysisResult, AnalysisError> {
        if !request.binary_path.is_file() {
            return Err(AnalysisError::MissingBinary(request.binary_path.clone()));
        }

        let rizin_path = request.backend_path.clone().unwrap_or_else(resolve_rizin_path);
        let version = version_string(&rizin_path).map_err(AnalysisError::Backend)?;

        // Allow tests to feed synthetic JSON via env to avoid needing rizin installed.
        let (functions, call_edges, mut evidence) =
            if let Some(fake_json) = std::env::var_os("BS_RIZIN_FAKE_JSON") {
                let body = fs::read_to_string(fake_json).map_err(|e| {
                    AnalysisError::Backend(format!("failed to read BS_RIZIN_FAKE_JSON: {e}"))
                })?;
                parse_functions(&body)?
            } else {
                let json = run_rizin_json(&rizin_path, &request.binary_path, "aa;aflj")?;
                parse_functions(&json)?
            };

        let basic_blocks = if let Some(fake_graph) = std::env::var_os("BS_RIZIN_FAKE_GRAPH") {
            let body = fs::read_to_string(fake_graph).map_err(|e| {
                AnalysisError::Backend(format!("failed to read BS_RIZIN_FAKE_GRAPH: {e}"))
            })?;
            parse_basic_blocks(&body)?
        } else {
            let json = run_rizin_json(&rizin_path, &request.binary_path, "aa;agfj")?;
            parse_basic_blocks(&json)?
        };

        evidence.push(EvidenceRecord {
            address: 0,
            description: version.clone(),
            kind: Some(crate::services::analysis::EvidenceKind::Other),
        });

        // Strings as evidence (optional).
        if request.options.include_strings {
            let strings_json = std::env::var_os("BS_RIZIN_FAKE_STRINGS")
                .map(|p| fs::read_to_string(&p).map_err(|e| e.to_string()))
                .transpose()
                .map_err(AnalysisError::Backend)?
                .unwrap_or_else(|| {
                    run_rizin_json(&rizin_path, &request.binary_path, "izj").unwrap_or_default()
                });
            if !strings_json.is_empty() {
                evidence.extend(parse_strings(&strings_json)?);
            }
        }

        // Imports as evidence (optional).
        if request.options.include_imports {
            let imports_json = std::env::var_os("BS_RIZIN_FAKE_IMPORTS")
                .map(|p| fs::read_to_string(&p).map_err(|e| e.to_string()))
                .transpose()
                .map_err(AnalysisError::Backend)?
                .unwrap_or_else(|| {
                    run_rizin_json(&rizin_path, &request.binary_path, "iij").unwrap_or_default()
                });
            if !imports_json.is_empty() {
                evidence.extend(parse_imports(&imports_json)?);
            }
        }

        Ok(AnalysisResult {
            functions,
            call_edges,
            evidence,
            basic_blocks,
            roots: request.roots.clone(),
            root_hits: crate::services::analysis::build_root_hits(&request.roots, &functions),
            backend_version: Some(version),
            backend_path: Some(rizin_path.display().to_string()),
        })
    }

    fn name(&self) -> &'static str {
        "rizin"
    }
}

fn resolve_rizin_path() -> PathBuf {
    std::env::var_os("RIZIN_BIN").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("rizin"))
}

fn run_rizin_json(rizin_bin: &Path, binary: &Path, command: &str) -> Result<String, AnalysisError> {
    let output = Command::new(rizin_bin)
        .args(["-2", "-q0", "-c", command])
        .arg(binary)
        .output()
        .map_err(|e| AnalysisError::Backend(format!("failed to spawn rizin: {e}")))?;
    if !output.status.success() {
        return Err(AnalysisError::Backend(format!("rizin exited with {}", output.status)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

fn parse_functions(
    body: &str,
) -> Result<(Vec<FunctionRecord>, Vec<CallEdge>, Vec<EvidenceRecord>), AnalysisError> {
    // rizin aflj returns a JSON array; tolerate empty/invalid gracefully.
    let funcs: Vec<RizinFunction> = serde_json::from_str(body)
        .map_err(|e| AnalysisError::Backend(format!("failed to parse rizin JSON: {e}")))?;
    let mut out_funcs = Vec::new();
    let mut edges = Vec::new();
    let mut evidence = Vec::new();
    for f in funcs {
        let from = f.offset.unwrap_or(0);
        out_funcs.push(FunctionRecord {
            address: from,
            name: f.name.clone(),
            size: f.size.map(|s| s as u32),
            in_slice: true,
            is_boundary: false,
        });
        if let Some(callrefs) = f.callrefs {
            for cref in callrefs {
                if cref.typ.as_deref() == Some("C") || cref.typ.as_deref() == Some("call") {
                    edges.push(CallEdge {
                        from,
                        to: cref.addr.unwrap_or(0),
                        is_cross_slice: false,
                    });
                    let desc = cref
                        .name
                        .map(|name| format!("call -> {}", name))
                        .unwrap_or_else(|| format!("call -> 0x{:X}", cref.addr.unwrap_or(0)));
                    evidence.push(EvidenceRecord {
                        address: from,
                        description: desc,
                        kind: Some(crate::services::analysis::EvidenceKind::Call),
                    });
                }
            }
        }
    }
    Ok((out_funcs, edges, evidence))
}

fn version_string(rizin_bin: &Path) -> Result<String, String> {
    if let Some(fake) = std::env::var_os("BS_RIZIN_FAKE_VERSION") {
        return Ok(fake.to_string_lossy().to_string());
    }
    let output = Command::new(rizin_bin)
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

#[derive(Debug, Deserialize)]
struct RizinFunction {
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    callrefs: Option<Vec<RizinCallRef>>,
}

#[derive(Debug, Deserialize)]
struct RizinCallRef {
    #[serde(default)]
    addr: Option<u64>,
    #[serde(default)]
    #[serde(rename = "type")]
    typ: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RizinGraphFunction {
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    blocks: Option<Vec<RizinBlock>>,
}

#[derive(Debug, Deserialize)]
struct RizinBlock {
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    jump: Option<u64>,
    #[serde(default)]
    fail: Option<u64>,
}

fn parse_basic_blocks(
    body: &str,
) -> Result<Vec<crate::services::analysis::BasicBlock>, AnalysisError> {
    let funcs: Vec<RizinGraphFunction> = serde_json::from_str(body)
        .map_err(|e| AnalysisError::Backend(format!("failed to parse rizin agfj JSON: {e}")))?;
    let mut blocks_out = Vec::new();
    for func in funcs {
        if let Some(blocks) = func.blocks {
            for b in blocks {
                let start = b.offset.unwrap_or(0);
                let mut successors = Vec::new();
                if let Some(j) = b.jump {
                    successors.push(crate::services::analysis::BlockEdge {
                        target: j,
                        kind: crate::services::analysis::BlockEdgeKind::Jump,
                    });
                }
                if let Some(f) = b.fail {
                    successors.push(crate::services::analysis::BlockEdge {
                        target: f,
                        kind: crate::services::analysis::BlockEdgeKind::ConditionalJump,
                    });
                }
                blocks_out.push(crate::services::analysis::BasicBlock {
                    start,
                    len: b.size.unwrap_or(0) as u32,
                    successors,
                });
            }
        }
    }
    Ok(blocks_out)
}

#[derive(Debug, Deserialize)]
struct RizinString {
    #[serde(default)]
    vaddr: Option<u64>,
    #[serde(default)]
    string: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RizinImport {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    plt: Option<u64>,
    #[serde(default)]
    bind: Option<String>,
}

fn parse_strings(body: &str) -> Result<Vec<EvidenceRecord>, AnalysisError> {
    let strs: Vec<RizinString> = serde_json::from_str(body)
        .map_err(|e| AnalysisError::Backend(format!("failed to parse rizin strings JSON: {e}")))?;
    Ok(strs
        .into_iter()
        .filter_map(|s| {
            s.string.as_ref().map(|text| EvidenceRecord {
                address: s.vaddr.unwrap_or(0),
                description: format!("string: {}", text),
                kind: Some(crate::services::analysis::EvidenceKind::String),
            })
        })
        .collect())
}

fn parse_imports(body: &str) -> Result<Vec<EvidenceRecord>, AnalysisError> {
    let imports: Vec<RizinImport> = serde_json::from_str(body)
        .map_err(|e| AnalysisError::Backend(format!("failed to parse rizin imports JSON: {e}")))?;
    Ok(imports
        .into_iter()
        .filter_map(|imp| {
            imp.name.as_ref().map(|name| EvidenceRecord {
                address: imp.plt.unwrap_or(0),
                description: format!("import: {}", name),
                kind: Some(crate::services::analysis::EvidenceKind::Import),
            })
        })
        .collect())
}
