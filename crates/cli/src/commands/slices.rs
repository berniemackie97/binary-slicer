use std::collections::HashMap;
use std::fs;

use crate::canonicalize_or_current;
use anyhow::{Context, Result};
use ritual_core::db::{RitualRunRecord, SliceRecord};
use ritual_core::services::analysis::{AnalysisResult, BlockEdgeKind, EvidenceKind};
use serde::Serialize;
use serde_json;
use serde_yaml;
use std::path::Path;

/// Initialize a new slice record and its documentation scaffold.
pub fn init_slice_command(
    root: &str,
    name: &str,
    description: Option<String>,
    default_binary: Option<String>,
) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Load project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = std::path::Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };
    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    // Insert slice record.
    let record = ritual_core::db::SliceRecord::new(name, ritual_core::db::SliceStatus::Planned)
        .with_description(description.clone())
        .with_default_binary(default_binary.clone());
    db.insert_slice(&record).context("Failed to insert slice record")?;

    // Create slice doc scaffold.
    fs::create_dir_all(&layout.slices_docs_dir).with_context(|| {
        format!("Failed to ensure slices docs dir {}", layout.slices_docs_dir.display())
    })?;
    let doc_path = layout.slices_docs_dir.join(format!("{name}.md"));
    let mut contents = String::new();
    contents.push_str(&format!("# {name}\n\n"));
    if let Some(desc) = description {
        contents.push_str(&desc);
        contents.push_str("\n\n");
    } else {
        contents.push_str("TODO: add a human-readable description of this slice.\n\n");
    }
    contents.push_str(
        "## Roots\n- TODO: list root functions (by address/name) that define this slice.\n\n",
    );
    contents.push_str("## Functions\n- TODO: populated by analysis runs.\n\n");
    contents.push_str(
        "## Evidence\n- TODO: xrefs, strings, patterns that justify membership in this slice.\n",
    );

    fs::write(&doc_path, contents)
        .with_context(|| format!("Failed to write slice doc at {}", doc_path.display()))?;

    println!("Initialized slice:");
    println!("  Name: {}", name);
    println!("  Root: {}", layout.root.display());
    println!("  Doc:  {}", doc_path.display());

    Ok(())
}

/// List all slices registered in the project database.
pub fn list_slices_command(root: &str, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Load project config so we know where the DB lives.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;

    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = std::path::Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    // Load DB metadata.
    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;
    let slices = db.list_slices().context("Failed to list slices")?;

    if json {
        let payload: Vec<serde_json::Value> = slices
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "default_binary": s.default_binary,
                    "status": format!("{:?}", s.status),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if slices.is_empty() {
        println!("Slices:");
        println!("(none)");
        return Ok(());
    }

    println!("Slices:");
    for slice in slices {
        let desc = slice.description.unwrap_or_else(|| "(no description)".to_string());
        let bin = slice.default_binary.as_deref().unwrap_or("(no default binary)");
        println!("- {} ({:?}) - {} [binary: {}]", slice.name, slice.status, desc, bin);
    }

    Ok(())
}

/// Regenerate slice docs for all slices in the DB.
pub fn emit_slice_docs_command(root: &str) -> Result<()> {
    use ritual_core::db::{ProjectConfig, ProjectDb, ProjectLayout};

    let root_path = canonicalize_or_current(root)?;
    let layout = ProjectLayout::new(&root_path);

    // Load project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = std::path::Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    fs::create_dir_all(&layout.slices_docs_dir).with_context(|| {
        format!("Failed to ensure slices docs dir {}", layout.slices_docs_dir.display())
    })?;

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    let runs = db.list_ritual_runs(None).unwrap_or_default();
    let slices = db.list_slices().context("Failed to list slices")?;
    if slices.is_empty() {
        println!("No slices to emit docs for.");
        return Ok(());
    }

    for slice in slices {
        let doc_path = layout.slices_docs_dir.join(format!("{}.md", slice.name));
        let mut contents = String::new();
        contents.push_str(&format!("# {}\n\n", slice.name));
        if let Some(desc) = &slice.description {
            contents.push_str(desc);
            contents.push_str("\n\n");
        } else {
            contents.push_str("TODO: add a human-readable description of this slice.\n\n");
        }
        if let Some(bin) = &slice.default_binary {
            contents.push_str(&format!("**Default binary:** {}\n\n", bin));
        }
        // Pull analysis (latest matching run) to populate functions/evidence if available.
        let latest_run = latest_run_for_slice(&slice, None, &runs);
        let analysis = latest_run
            .and_then(|run| db.load_analysis_result(&run.binary, &run.ritual).ok())
            .flatten();
        let roots = analysis
            .as_ref()
            .map(|a| a.roots.clone())
            .filter(|r| !r.is_empty())
            .or_else(|| latest_run.map(|run| load_roots_for_run(&layout, run)))
            .unwrap_or_default();
        let backend_version = analysis
            .as_ref()
            .and_then(|a| a.backend_version.clone())
            .or_else(|| latest_run.and_then(|r| r.backend_version.clone()));
        let backend_path = analysis
            .as_ref()
            .and_then(|a| a.backend_path.clone())
            .or_else(|| latest_run.and_then(|r| r.backend_path.clone()));
        let mapping =
            analysis.as_ref().map(|a| map_evidence_to_functions(&a.functions, &a.evidence));
        let summary = analysis.as_ref().map(|a| summarize_analysis(a, roots.len()));

        if let Some(run) = latest_run {
            contents.push_str("**Backend:** ");
            contents.push_str(&run.backend);
            if let Some(v) = &backend_version {
                contents.push_str(&format!(" {}", v));
            }
            if let Some(p) = &backend_path {
                contents.push_str(&format!(" @ {}", p));
            }
            contents.push_str("\n\n");
        }

        if let Some(summary) = &summary {
            contents.push_str("## Summary\n");
            contents.push_str(&format!(
                "- Functions: {} (in-slice={}, boundary={})\n",
                summary.functions, summary.functions_in_slice, summary.boundary_functions
            ));
            contents.push_str(&format!(
                "- Call edges: {} (cross-slice={})\n",
                summary.call_edges, summary.cross_slice_calls
            ));
            contents.push_str(&format!("- Basic blocks: {}\n", summary.basic_blocks));
            contents.push_str(&format!("- Roots: {}\n", summary.roots));
            contents.push_str(&format!(
                "- Evidence: total={} strings={} imports={} calls={} other={}\n\n",
                summary.evidence.total,
                summary.evidence.strings,
                summary.evidence.imports,
                summary.evidence.calls,
                summary.evidence.other
            ));
        }

        if latest_run.is_some() {
            contents.push_str("## Roots\n");
            if roots.is_empty() {
                contents.push_str("- (no roots recorded)\n\n");
            } else {
                for r in &roots {
                    contents.push_str(&format!("- {}\n", r));
                }
                contents.push('\n');
            }
        } else {
            contents.push_str(
                "## Roots\n- TODO: list root functions (by address/name) that define this slice.\n\n",
            );
        }
        contents.push_str("## Functions\n");
        if let Some(a) = &analysis {
            if a.functions.is_empty() {
                contents.push_str("- (no functions recorded)\n\n");
            } else {
                for f in &a.functions {
                    let label = f.name.clone().unwrap_or_else(|| format!("0x{:X}", f.address));
                    let mut tags = Vec::new();
                    if let Some(size) = f.size {
                        tags.push(format!("size={}", size));
                    }
                    if f.in_slice {
                        tags.push("in-slice".into());
                    }
                    if f.is_boundary {
                        tags.push("boundary".into());
                    }
                    let func_evidence = mapping
                        .as_ref()
                        .and_then(|m| m.by_function.get(&f.address))
                        .cloned()
                        .unwrap_or_default();
                    let func_buckets = categorize_evidence(&func_evidence);
                    contents.push_str(&format!("- {} @ 0x{:X}", label, f.address));
                    if !tags.is_empty() {
                        contents.push_str(&format!(" ({})", tags.join(", ")));
                    }
                    if func_buckets.total() > 0 {
                        contents.push_str(&format!(
                            " — evidence: total={} strings={} imports={} calls={} other={}",
                            func_buckets.total(),
                            func_buckets.strings.len(),
                            func_buckets.imports.len(),
                            func_buckets.calls.len(),
                            func_buckets.other.len()
                        ));
                    }
                    contents.push('\n');
                    if !func_evidence.is_empty() {
                        write_inline_evidence(&mut contents, &func_evidence, 5);
                        contents.push('\n');
                    }
                }
            }
        } else {
            contents.push_str("- TODO: populated by analysis runs.\n\n");
        }

        contents.push_str("## Evidence\n");
        if let Some(a) = &analysis {
            if a.evidence.is_empty() {
                contents.push_str("- (no evidence recorded)\n");
            } else {
                let categorized = categorize_evidence(&a.evidence);
                contents.push_str(&format!(
                    "- Summary: total={} strings={} imports={} calls={} other={}\n\n",
                    categorized.total(),
                    categorized.strings.len(),
                    categorized.imports.len(),
                    categorized.calls.len(),
                    categorized.other.len()
                ));
                write_evidence_section(&mut contents, "Strings", &categorized.strings, 15);
                write_evidence_section(&mut contents, "Imports", &categorized.imports, 15);
                write_evidence_section(&mut contents, "Calls", &categorized.calls, 15);
                write_evidence_section(&mut contents, "Other evidence", &categorized.other, 15);
                if let Some(m) = &mapping {
                    if !m.unmapped.is_empty() {
                        write_evidence_section(
                            &mut contents,
                            "Unmapped evidence (no matching function)",
                            &m.unmapped,
                            15,
                        );
                    }
                }
            }
        } else {
            contents.push_str(
                "- TODO: xrefs, strings, patterns that justify membership in this slice.\n",
            );
        }

        fs::write(&doc_path, contents)
            .with_context(|| format!("Failed to write slice doc at {}", doc_path.display()))?;
        println!("Emitted slice doc: {}", doc_path.display());
    }

    Ok(())
}

/// Regenerate slice reports for all slices in the DB.
pub fn emit_slice_reports_command(root: &str, preferred_binary: Option<&str>) -> Result<()> {
    use ritual_core::db::{ProjectConfig, ProjectDb, ProjectLayout};

    let root_path = canonicalize_or_current(root)?;
    let layout = ProjectLayout::new(&root_path);

    // Load project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = std::path::Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    fs::create_dir_all(&layout.reports_dir).with_context(|| {
        format!("Failed to ensure reports dir {}", layout.reports_dir.display())
    })?;
    fs::create_dir_all(&layout.graphs_dir)
        .with_context(|| format!("Failed to ensure graphs dir {}", layout.graphs_dir.display()))?;

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    let slices = db.list_slices().context("Failed to list slices")?;
    if slices.is_empty() {
        println!("No slices to emit reports for.");
        return Ok(());
    }

    for slice in slices {
        let report_path = layout.reports_dir.join(format!("{}.json", slice.name));
        let graph_path = layout.graphs_dir.join(format!("{}.dot", slice.name));

        // Heuristic: use the latest ritual run whose name matches the slice name.
        let all_runs = db.list_ritual_runs(None).unwrap_or_default();
        let latest_run = latest_run_for_slice(&slice, preferred_binary, &all_runs);
        let analysis = latest_run
            .and_then(|run| db.load_analysis_result(&run.binary, &run.ritual).ok())
            .flatten();
        let roots = analysis
            .as_ref()
            .map(|a| a.roots.clone())
            .filter(|r| !r.is_empty())
            .or_else(|| latest_run.map(|run| load_roots_for_run(&layout, run)))
            .unwrap_or_default();

        let (functions, call_edges, evidence, basic_blocks) = if let Some(a) = &analysis {
            (
                serde_json::to_value(&a.functions)?,
                serde_json::to_value(&a.call_edges)?,
                serde_json::to_value(&a.evidence)?,
                serde_json::to_value(&a.basic_blocks)?,
            )
        } else {
            (
                serde_json::json!([]),
                serde_json::json!([]),
                serde_json::json!([]),
                serde_json::json!([]),
            )
        };
        let backend = latest_run.map(|r| r.backend.clone());
        let backend_version = analysis
            .as_ref()
            .and_then(|a| a.backend_version.clone())
            .or_else(|| latest_run.and_then(|r| r.backend_version.clone()));
        let backend_path = analysis
            .as_ref()
            .and_then(|a| a.backend_path.clone())
            .or_else(|| latest_run.and_then(|r| r.backend_path.clone()));
        let categorized = analysis.as_ref().map(|a| categorize_evidence(&a.evidence));
        let mapping =
            analysis.as_ref().map(|a| map_evidence_to_functions(&a.functions, &a.evidence));
        let summary = analysis.as_ref().map(|a| summarize_analysis(a, roots.len()));
        let function_evidence = analysis
            .as_ref()
            .and_then(|a| mapping.as_ref().map(|m| build_function_evidence_json(&a.functions, m)));

        let report = serde_json::json!({
            "name": slice.name,
            "description": slice.description,
            "status": format!("{:?}", slice.status),
            "roots": roots,
            "functions": functions,
            "call_edges": call_edges,
            "basic_blocks": basic_blocks,
            "evidence": evidence,
            "evidence_counts": categorized.as_ref().map(|c| c.counts()),
            "strings": categorized.as_ref().map(|c| c.strings.clone()).unwrap_or_default(),
            "imports": categorized.as_ref().map(|c| c.imports.clone()).unwrap_or_default(),
            "calls": categorized.as_ref().map(|c| c.calls.clone()).unwrap_or_default(),
            "other_evidence": categorized.as_ref().map(|c| c.other.clone()).unwrap_or_default(),
            "backend": backend,
            "backend_version": backend_version,
            "backend_path": backend_path,
            "analysis_summary": summary,
            "function_evidence": function_evidence,
        });
        let serialized = serde_json::to_string_pretty(&report)?;
        fs::write(&report_path, serialized).with_context(|| {
            format!("Failed to write slice report at {}", report_path.display())
        })?;
        println!("Emitted slice report: {}", report_path.display());

        let dot = render_dot_from_analysis(analysis.as_ref(), backend, backend_version.as_deref());
        fs::write(&graph_path, dot)
            .with_context(|| format!("Failed to write slice graph at {}", graph_path.display()))?;
        println!("Emitted slice graph: {}", graph_path.display());
    }

    Ok(())
}

fn render_dot_from_analysis(
    analysis: Option<&AnalysisResult>,
    backend: Option<String>,
    backend_version: Option<&str>,
) -> String {
    let mut out = String::from("digraph Slice {\n  rankdir=LR;\n");
    if let Some(b) = backend {
        let mut label = format!("backend: {}", b);
        if let Some(v) = backend_version {
            label.push_str(&format!(" {}", v));
        }
        let safe_label = label.replace('"', "\\\"");
        out.push_str(&format!("  label=\"{}\";\n  labelloc=top;\n", safe_label));
    }
    if let Some(result) = analysis {
        for func in &result.functions {
            let label = func.name.clone().unwrap_or_else(|| format!("0x{:X}", func.address));
            out.push_str(&format!("  f_{:X} [label=\"{}\" shape=box];\n", func.address, label));
        }
        for edge in &result.call_edges {
            out.push_str(&format!("  f_{:X} -> f_{:X} [label=\"call\"];\n", edge.from, edge.to));
        }
        for bb in &result.basic_blocks {
            out.push_str(&format!(
                "  bb_{:X} [label=\"bb 0x{:X}\\nlen={}\" shape=ellipse];\n",
                bb.start, bb.start, bb.len
            ));
            for succ in &bb.successors {
                let label = match succ.kind {
                    BlockEdgeKind::Fallthrough => "fallthrough",
                    BlockEdgeKind::Jump => "jump",
                    BlockEdgeKind::ConditionalJump => "cjump",
                    BlockEdgeKind::IndirectJump => "ijump",
                    BlockEdgeKind::Call => "call",
                    BlockEdgeKind::IndirectCall => "icall",
                };
                out.push_str(&format!(
                    "  bb_{:X} -> bb_{:X} [label=\"{}\"];\n",
                    bb.start, succ.target, label
                ));
            }
        }
    } else {
        out.push_str("  // no analysis available for this slice\n");
    }
    out.push_str("}\n");
    out
}

fn latest_run_for_slice<'a>(
    slice: &SliceRecord,
    preferred_binary: Option<&str>,
    runs: &'a [RitualRunRecord],
) -> Option<&'a RitualRunRecord> {
    let filtered: Vec<&RitualRunRecord> = runs
        .iter()
        .filter(|r| {
            r.ritual == slice.name
                && preferred_binary
                    .map(|b| r.binary == b)
                    .or_else(|| slice.default_binary.as_ref().map(|b| r.binary == *b))
                    .unwrap_or(true)
        })
        .collect();
    if !filtered.is_empty() {
        filtered
            .into_iter()
            .max_by(|a, b| a.finished_at.cmp(&b.finished_at).then(a.started_at.cmp(&b.started_at)))
    } else {
        runs.iter()
            .filter(|r| r.ritual == slice.name)
            .max_by(|a, b| a.finished_at.cmp(&b.finished_at).then(a.started_at.cmp(&b.started_at)))
    }
}

fn load_roots_for_run(
    layout: &ritual_core::db::ProjectLayout,
    run: &RitualRunRecord,
) -> Vec<String> {
    let run_dir = layout.binary_output_root(&run.binary).join(&run.ritual);
    let spec_path = run_dir.join("spec.yaml");
    if !spec_path.is_file() {
        return Vec::new();
    }
    parse_roots_from_spec(&spec_path).unwrap_or_default()
}

fn parse_roots_from_spec(path: &Path) -> Option<Vec<String>> {
    let body = std::fs::read_to_string(path).ok()?;
    let mut roots: Option<Vec<String>> = serde_yaml::from_str::<serde_yaml::Value>(&body)
        .ok()
        .and_then(|v| v.get("roots").cloned())
        .and_then(value_to_strings);
    if roots.is_none() {
        roots = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("roots").cloned())
            .and_then(json_value_to_strings);
    }
    roots
}

fn value_to_strings(value: serde_yaml::Value) -> Option<Vec<String>> {
    let seq = value.as_sequence()?;
    let mut out = Vec::new();
    for item in seq {
        if let Some(s) = item.as_str() {
            out.push(s.to_string());
        }
    }
    Some(out)
}

fn json_value_to_strings(value: serde_json::Value) -> Option<Vec<String>> {
    let seq = value.as_array()?;
    let mut out = Vec::new();
    for item in seq {
        if let Some(s) = item.as_str() {
            out.push(s.to_string());
        }
    }
    Some(out)
}

#[derive(Clone)]
struct EvidenceBuckets {
    strings: Vec<ritual_core::services::analysis::EvidenceRecord>,
    imports: Vec<ritual_core::services::analysis::EvidenceRecord>,
    calls: Vec<ritual_core::services::analysis::EvidenceRecord>,
    other: Vec<ritual_core::services::analysis::EvidenceRecord>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EvidenceCounts {
    total: usize,
    strings: usize,
    imports: usize,
    calls: usize,
    other: usize,
}

impl EvidenceBuckets {
    fn total(&self) -> usize {
        self.strings.len() + self.imports.len() + self.calls.len() + self.other.len()
    }
    fn as_counts(&self) -> EvidenceCounts {
        EvidenceCounts {
            total: self.total(),
            strings: self.strings.len(),
            imports: self.imports.len(),
            calls: self.calls.len(),
            other: self.other.len(),
        }
    }
    fn counts(&self) -> serde_json::Value {
        serde_json::json!({
            "total": self.total(),
            "strings": self.strings.len(),
            "imports": self.imports.len(),
            "calls": self.calls.len(),
            "other": self.other.len(),
        })
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct AnalysisSummary {
    functions: usize,
    functions_in_slice: usize,
    boundary_functions: usize,
    call_edges: usize,
    cross_slice_calls: usize,
    basic_blocks: usize,
    roots: usize,
    evidence: EvidenceCounts,
}

fn categorize_evidence(
    evidence: &[ritual_core::services::analysis::EvidenceRecord],
) -> EvidenceBuckets {
    let mut buckets = EvidenceBuckets {
        strings: Vec::new(),
        imports: Vec::new(),
        calls: Vec::new(),
        other: Vec::new(),
    };
    for e in evidence {
        match e.kind {
            Some(EvidenceKind::String) => buckets.strings.push(e.clone()),
            Some(EvidenceKind::Import) => buckets.imports.push(e.clone()),
            Some(EvidenceKind::Call) => buckets.calls.push(e.clone()),
            _ => buckets.other.push(e.clone()),
        }
    }
    buckets
}

#[derive(Default)]
struct EvidenceMapping {
    by_function: HashMap<u64, Vec<ritual_core::services::analysis::EvidenceRecord>>,
    unmapped: Vec<ritual_core::services::analysis::EvidenceRecord>,
}

fn summarize_analysis(analysis: &AnalysisResult, roots: usize) -> AnalysisSummary {
    let evidence = categorize_evidence(&analysis.evidence);
    let functions_in_slice = analysis.functions.iter().filter(|f| f.in_slice).count();
    let boundary_functions = analysis.functions.iter().filter(|f| f.is_boundary).count();
    let cross_slice_calls = analysis.call_edges.iter().filter(|e| e.is_cross_slice).count();
    AnalysisSummary {
        functions: analysis.functions.len(),
        functions_in_slice,
        boundary_functions,
        call_edges: analysis.call_edges.len(),
        cross_slice_calls,
        basic_blocks: analysis.basic_blocks.len(),
        roots,
        evidence: evidence.as_counts(),
    }
}

fn map_evidence_to_functions(
    functions: &[ritual_core::services::analysis::FunctionRecord],
    evidence: &[ritual_core::services::analysis::EvidenceRecord],
) -> EvidenceMapping {
    let mut mapping = EvidenceMapping::default();
    for ev in evidence {
        if let Some(addr) = find_function_for_evidence(functions, ev.address) {
            mapping.by_function.entry(addr).or_default().push(ev.clone());
        } else {
            mapping.unmapped.push(ev.clone());
        }
    }
    mapping
}

fn find_function_for_evidence(
    functions: &[ritual_core::services::analysis::FunctionRecord],
    addr: u64,
) -> Option<u64> {
    let mut best: Option<(u64, u64)> = None;
    for func in functions {
        if let Some(size) = func.size {
            let start = func.address;
            let end = start.saturating_add(size as u64);
            if addr >= start && addr < end {
                let span = end.saturating_sub(start);
                if best.map(|(_, s)| span < s).unwrap_or(true) {
                    best = Some((func.address, span));
                }
            }
        } else if addr == func.address && best.is_none() {
            best = Some((func.address, u64::MAX));
        }
    }
    best.map(|(a, _)| a)
}

fn write_evidence_section(
    buf: &mut String,
    heading: &str,
    items: &[ritual_core::services::analysis::EvidenceRecord],
    limit: usize,
) {
    if items.is_empty() {
        return;
    }
    buf.push_str(&format!("### {}\n", heading));
    for e in items.iter().take(limit) {
        buf.push_str(&format!("- 0x{:X}: {}\n", e.address, e.description));
    }
    if items.len() > limit {
        buf.push_str(&format!("- ... ({} more {})\n", items.len() - limit, heading.to_lowercase()));
    }
    buf.push('\n');
}

fn write_inline_evidence(
    buf: &mut String,
    items: &[ritual_core::services::analysis::EvidenceRecord],
    limit: usize,
) {
    for e in items.iter().take(limit) {
        buf.push_str(&format!("  - 0x{:X}: {}\n", e.address, e.description));
    }
    if items.len() > limit {
        buf.push_str(&format!("  - ... ({} more entries)\n", items.len() - limit));
    }
}

fn build_function_evidence_json(
    functions: &[ritual_core::services::analysis::FunctionRecord],
    mapping: &EvidenceMapping,
) -> serde_json::Value {
    let mut by_function = serde_json::Map::new();
    for func in functions {
        if let Some(items) = mapping.by_function.get(&func.address) {
            let counts = categorize_evidence(items);
            by_function.insert(
                format!("0x{:X}", func.address),
                serde_json::json!({
                    "function": func,
                    "evidence": items,
                    "evidence_counts": counts.counts(),
                }),
            );
        }
    }
    serde_json::json!({
        "by_function": serde_json::Value::Object(by_function),
        "unmapped": mapping.unmapped.clone(),
    })
}
