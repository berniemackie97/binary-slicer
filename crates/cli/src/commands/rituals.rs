use std::fs;
use std::path::Path;

use crate::canonicalize_or_current;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use ritual_core::db::RitualRunStatus;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;

use crate::commands::{
    collect_ritual_specs, load_runs_from_db, load_runs_from_db_and_disk, open_project_db,
    validate_run_status,
};
use ritual_core::services::analysis::{
    default_backend_registry, AnalysisOptions, AnalysisRequest, RitualRunner, RunMetadata,
};

const DEFAULT_BACKEND_NAME: &str = "validate-only";

fn default_backend_name() -> String {
    DEFAULT_BACKEND_NAME.to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RitualSpec {
    pub name: String,
    pub binary: String,
    pub roots: Vec<String>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub outputs: Option<RitualOutputs>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RitualOutputs {
    #[serde(default)]
    pub reports: bool,
    #[serde(default)]
    pub graphs: bool,
    #[serde(default)]
    pub docs: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RitualRunMetadata {
    pub ritual: String,
    pub binary: String,
    pub spec_hash: String,
    pub binary_hash: Option<String>,
    #[serde(default = "default_backend_name")]
    pub backend: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: RitualRunStatus,
}

#[derive(Debug, Serialize, Clone)]
pub struct RitualRunInfo {
    pub binary: String,
    pub name: String,
    pub path: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: Option<String>,
    pub spec_hash: Option<String>,
    pub backend: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RitualSpecInfo {
    pub name: String,
    pub binary: Option<String>,
    pub path: String,
    pub format: String,
}

impl RitualSpec {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow!("Ritual spec 'name' is required"));
        }
        if self.binary.trim().is_empty() {
            return Err(anyhow!("Ritual spec 'binary' is required"));
        }
        if self.roots.is_empty() {
            return Err(anyhow!("Ritual spec must include at least one root"));
        }
        Ok(())
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn db_run_to_info(
    layout: &ritual_core::db::ProjectLayout,
    rec: &ritual_core::db::RitualRunRecord,
) -> RitualRunInfo {
    RitualRunInfo {
        binary: rec.binary.clone(),
        name: rec.ritual.clone(),
        path: layout.binary_output_root(&rec.binary).join(&rec.ritual).display().to_string(),
        started_at: Some(rec.started_at.clone()),
        finished_at: Some(rec.finished_at.clone()),
        status: Some(rec.status.as_str().to_string()),
        spec_hash: Some(rec.spec_hash.clone()),
        backend: Some(rec.backend.clone()),
    }
}

/// Run a ritual spec (stub analysis) and organize outputs per binary and ritual name.
pub fn run_ritual_command(
    root: &str,
    file: &str,
    backend_override: Option<&str>,
    force: bool,
) -> Result<()> {
    use ritual_core::db::ProjectLayout;

    let root_path = canonicalize_or_current(root)?;
    let layout = ProjectLayout::new(&root_path);

    let (config, db_path, db) = open_project_db(&layout)?;

    // Load ritual spec (supports YAML or JSON based on extension).
    let spec_path = Path::new(file);
    let spec_bytes = fs::read(spec_path)
        .with_context(|| format!("Failed to read ritual spec at {}", spec_path.display()))?;
    let spec_hash = sha256_bytes(&spec_bytes);
    let spec: RitualSpec = if spec_path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_slice(&spec_bytes).context("Failed to parse ritual spec JSON")?
    } else {
        serde_yaml::from_slice(&spec_bytes).context("Failed to parse ritual spec YAML")?
    };
    spec.validate()?;

    // Make sure the binary exists in the DB.
    let binaries = db.list_binaries().context("Failed to list binaries")?;
    let target_bin = binaries
        .iter()
        .find(|b| b.name == spec.binary || b.path.ends_with(&spec.binary))
        .cloned()
        .ok_or_else(|| anyhow!("Binary '{}' not found in project database", spec.binary))?;

    // Prepare output directories.
    let bin_output_root = layout.binary_output_root(&target_bin.name);
    let run_output_root = bin_output_root.join(&spec.name);
    if run_output_root.exists() {
        if force {
            fs::remove_dir_all(&run_output_root).with_context(|| {
                format!("Failed to clean existing ritual output dir {}", run_output_root.display())
            })?;
        } else {
            return Err(anyhow!(
                "Ritual output already exists at {} (rerun with --force to overwrite)",
                run_output_root.display()
            ));
        }
    }
    fs::create_dir_all(&run_output_root).with_context(|| {
        format!("Failed to create ritual output dir {}", run_output_root.display())
    })?;

    // Resolve binary hash (prefer stored hash; compute if missing).
    let binary_path = {
        let p = Path::new(&target_bin.path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root_path.join(p)
        }
    };
    let binary_hash = if let Some(h) = &target_bin.hash {
        Some(h.clone())
    } else if binary_path.exists() {
        Some(crate::sha256_file(&binary_path)?)
    } else {
        None
    };

    // Choose backend (CLI override > spec > default), then persist normalized spec copy.
    let backend_name = backend_override
        .map(|s| s.to_string())
        .or_else(|| config.default_backend.clone())
        .or_else(|| spec.backend.clone())
        .unwrap_or_else(default_backend_name);
    let mut spec_copy = spec;
    if spec_copy.outputs.is_none() {
        spec_copy.outputs = Some(RitualOutputs { reports: true, graphs: true, docs: true });
    }
    spec_copy.backend = Some(backend_name.clone());
    let normalized_spec_path = run_output_root.join("spec.yaml");
    let yaml = serde_yaml::to_string(&spec_copy).context("Failed to serialize ritual spec")?;
    fs::write(&normalized_spec_path, yaml).with_context(|| {
        format!("Failed to write normalized spec to {}", normalized_spec_path.display())
    })?;

    // Invoke analysis service (validate-only default backend for now).
    let backends = default_backend_registry();
    let backend = backends.get(&backend_name).ok_or_else(|| {
        anyhow!("Backend '{}' not found (available: {:?})", backend_name, backends.names())
    })?;
    let request = AnalysisRequest {
        ritual_name: spec_copy.name.clone(),
        binary_name: target_bin.name.clone(),
        binary_path: binary_path.clone(),
        roots: spec_copy.roots.clone(),
        options: AnalysisOptions {
            max_depth: spec_copy.max_depth,
            include_imports: false,
            include_strings: false,
        },
    };
    let ctx = ritual_core::db::ProjectContext {
        layout: layout.clone(),
        config,
        db_path: db_path.clone(),
        db,
    };
    let runner = RitualRunner { ctx: &ctx, backend };
    let run_meta = RunMetadata {
        spec_hash: spec_hash.clone(),
        binary_hash: binary_hash.clone(),
        backend: backend_name.clone(),
        status: RitualRunStatus::Stubbed,
    };
    let analysis_result = runner.run(&request, &run_meta)?;

    // Write report from analysis result.
    let report_path = run_output_root.join("report.json");
    let report = serde_json::json!({
        "ritual": spec_copy.name,
        "binary": target_bin.name,
        "roots": spec_copy.roots,
        "max_depth": spec_copy.max_depth,
        "status": run_meta.status.as_str(),
        "backend": backend_name,
        "functions": analysis_result.functions,
        "edges": analysis_result.call_edges,
        "evidence": analysis_result.evidence,
    });
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("Failed to write ritual report at {}", report_path.display()))?;

    // Write run metadata.
    let now = Utc::now().to_rfc3339();
    let metadata = RitualRunMetadata {
        ritual: spec_copy.name.clone(),
        binary: target_bin.name.clone(),
        spec_hash,
        binary_hash,
        backend: backend_name,
        started_at: now.clone(),
        finished_at: now,
        status: run_meta.status,
    };
    let metadata_path = run_output_root.join("run_metadata.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)
        .with_context(|| format!("Failed to write run metadata at {}", metadata_path.display()))?;

    println!("Ran ritual (stub): {}", spec_copy.name);
    println!("  Binary: {}", target_bin.name);
    println!("  Roots: {:?}", spec_copy.roots);
    println!("  Output: {}", run_output_root.display());

    Ok(())
}

/// Rerun a ritual by reusing a normalized spec from an existing run.
pub fn rerun_ritual_command(
    root: &str,
    binary: &str,
    ritual: &str,
    as_name: &str,
    backend_override: Option<&str>,
    force: bool,
) -> Result<()> {
    use ritual_core::db::ProjectLayout;

    let root_path = canonicalize_or_current(root)?;
    let layout = ProjectLayout::new(&root_path);

    let (config, db_path, db) = open_project_db(&layout)?;

    // Verify binary exists in DB.
    let binaries = db.list_binaries().context("Failed to list binaries")?;
    let target_bin = binaries
        .iter()
        .find(|b| b.name == binary || b.path.ends_with(binary))
        .cloned()
        .ok_or_else(|| anyhow!("Binary '{}' not found in project database", binary))?;

    // Locate existing run's spec.yaml.
    let existing_run_root = layout.binary_output_root(&target_bin.name).join(ritual);
    let existing_spec = existing_run_root.join("spec.yaml");
    if !existing_spec.is_file() {
        return Err(anyhow!("Spec not found for existing run at {}", existing_spec.display()));
    }

    // Deserialize spec.yaml.
    let spec_bytes = fs::read(&existing_spec)
        .with_context(|| format!("Failed to read existing spec at {}", existing_spec.display()))?;
    let spec_hash = sha256_bytes(&spec_bytes);
    let mut spec: RitualSpec =
        serde_yaml::from_slice(&spec_bytes).context("Failed to parse spec")?;
    spec.validate()?;

    // Prepare output dirs for new run.
    let new_run_root = layout.binary_output_root(&target_bin.name).join(as_name);
    if new_run_root.exists() {
        if force {
            fs::remove_dir_all(&new_run_root).with_context(|| {
                format!("Failed to clean existing ritual output dir {}", new_run_root.display())
            })?;
        } else {
            return Err(anyhow!(
                "Rerun output already exists at {} (use --force to overwrite)",
                new_run_root.display()
            ));
        }
    }
    fs::create_dir_all(&new_run_root)
        .with_context(|| format!("Failed to create rerun dir {}", new_run_root.display()))?;

    // Hash binary path.
    let binary_path = {
        let p = Path::new(&target_bin.path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root_path.join(p)
        }
    };
    let binary_hash = if let Some(h) = &target_bin.hash {
        Some(h.clone())
    } else if binary_path.exists() {
        Some(crate::sha256_file(&binary_path)?)
    } else {
        None
    };

    // Choose backend (CLI override > spec > default), then write normalized spec copy.
    let backend_name = backend_override
        .map(|s| s.to_string())
        .or_else(|| config.default_backend.clone())
        .or_else(|| spec.backend.clone())
        .unwrap_or_else(default_backend_name);
    if spec.outputs.is_none() {
        spec.outputs = Some(RitualOutputs { reports: true, graphs: true, docs: true });
    }
    spec.backend = Some(backend_name.clone());
    let normalized_spec_path = new_run_root.join("spec.yaml");
    let yaml = serde_yaml::to_string(&spec).context("Failed to serialize ritual spec")?;
    fs::write(&normalized_spec_path, yaml).with_context(|| {
        format!("Failed to write normalized spec to {}", normalized_spec_path.display())
    })?;

    // Invoke analysis service (validate-only default backend for now).
    let backends = default_backend_registry();
    let backend = backends.get(&backend_name).ok_or_else(|| {
        anyhow!("Backend '{}' not found (available: {:?})", backend_name, backends.names())
    })?;
    let request = AnalysisRequest {
        ritual_name: as_name.to_string(),
        binary_name: target_bin.name.clone(),
        binary_path: binary_path.clone(),
        roots: spec.roots.clone(),
        options: AnalysisOptions {
            max_depth: spec.max_depth,
            include_imports: false,
            include_strings: false,
        },
    };
    let ctx = ritual_core::db::ProjectContext {
        layout: layout.clone(),
        config,
        db_path: db_path.clone(),
        db,
    };
    let runner = RitualRunner { ctx: &ctx, backend };
    let run_meta = RunMetadata {
        spec_hash: spec_hash.clone(),
        binary_hash: binary_hash.clone(),
        backend: backend_name.clone(),
        status: RitualRunStatus::Stubbed,
    };
    let analysis_result = runner.run(&request, &run_meta)?;

    // Write report from analysis result.
    let report_path = new_run_root.join("report.json");
    let report = serde_json::json!({
        "ritual": as_name,
        "binary": target_bin.name,
        "roots": spec.roots,
        "max_depth": spec.max_depth,
        "status": run_meta.status.as_str(),
        "backend": backend_name,
        "functions": analysis_result.functions,
        "edges": analysis_result.call_edges,
        "evidence": analysis_result.evidence,
    });
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("Failed to write ritual report at {}", report_path.display()))?;

    // Write metadata for rerun.
    let now = Utc::now().to_rfc3339();
    let metadata = RitualRunMetadata {
        ritual: as_name.to_string(),
        binary: target_bin.name.clone(),
        spec_hash,
        binary_hash,
        backend: backend_name,
        started_at: now.clone(),
        finished_at: now,
        status: RitualRunStatus::Stubbed,
    };
    let metadata_path = new_run_root.join("run_metadata.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)
        .with_context(|| format!("Failed to write run metadata at {}", metadata_path.display()))?;

    println!("Reran ritual (stub): {} -> {}", ritual, as_name);
    println!("  Binary: {}", target_bin.name);
    println!("  Output: {}", new_run_root.display());

    Ok(())
}

/// Clean ritual outputs (per binary or per run) with confirmation gating.
pub fn clean_outputs_command(
    root: &str,
    binary: Option<&str>,
    ritual: Option<&str>,
    all: bool,
    yes: bool,
) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    if !yes {
        return Err(anyhow!("Refusing to delete outputs without --yes"));
    }

    if ritual.is_some() && binary.is_none() {
        return Err(anyhow!("--ritual requires --binary"));
    }

    if !all && binary.is_none() {
        return Err(anyhow!("Specify --binary or use --all to clean all outputs"));
    }

    let target_paths: Vec<(String, std::path::PathBuf)> = if all {
        vec![("all outputs".to_string(), layout.outputs_binaries_dir.clone())]
    } else if let Some(bin) = binary {
        let bin_root = layout.binary_output_root(bin);
        if let Some(rit) = ritual {
            vec![(format!("{} / {}", bin, rit), bin_root.join(rit))]
        } else {
            vec![(bin.to_string(), bin_root)]
        }
    } else {
        vec![]
    };

    for (label, path) in target_paths {
        if path.exists() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to remove outputs for {}", label))?;
            println!("Removed outputs: {}", path.display());
        } else {
            println!("Nothing to remove for {} ({} not found)", label, path.display());
        }
    }

    Ok(())
}

/// List ritual runs discovered under outputs/binaries (human or JSON).
pub fn list_ritual_runs_command(root: &str, binary_filter: Option<&str>, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    let runs = load_runs_from_db_and_disk(&layout, binary_filter)?;

    if json {
        let serialized = serde_json::to_string_pretty(&runs)?;
        println!("{}", serialized);
        return Ok(());
    }

    if runs.is_empty() {
        println!("Ritual runs: (none)");
        return Ok(());
    }

    println!("Ritual runs:");
    for run in runs {
        println!("- {} / {} -> {}", run.binary, run.name, run.path);
    }
    Ok(())
}

/// Show details for a single ritual run.
pub fn show_ritual_run_command(root: &str, binary: &str, ritual: &str, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);
    let run_root = layout.binary_output_root(binary).join(ritual);

    // Load DB metadata if present.
    let db_runs = load_runs_from_db(&layout, Some(binary)).unwrap_or_default();
    let db_run = db_runs.into_iter().find(|r| r.ritual == ritual);

    // Fallback to on-disk metadata if DB is missing the run.
    let spec_path = run_root.join("spec.yaml");
    let report_path = run_root.join("report.json");
    let metadata_path = run_root.join("run_metadata.json");
    let disk_metadata: Option<RitualRunMetadata> = if metadata_path.exists() {
        Some(
            serde_json::from_str(
                &fs::read_to_string(&metadata_path)
                    .with_context(|| format!("Failed to read {}", metadata_path.display()))?,
            )
            .with_context(|| format!("Failed to parse {}", metadata_path.display()))?,
        )
    } else {
        None
    };

    if db_run.is_none() && !run_root.is_dir() {
        return Err(anyhow!("Ritual run not found in DB or at {}", run_root.display()));
    }

    if json {
        let payload = if let Some(run) = db_run.clone() {
            serde_json::json!({
                "binary": run.binary,
                "ritual": run.ritual,
                "path": run_root.display().to_string(),
                "spec": spec_path.display().to_string(),
                "report": report_path.display().to_string(),
                "metadata": {
                    "spec_hash": run.spec_hash,
                    "binary_hash": run.binary_hash,
                    "backend": run.backend,
                    "status": run.status.as_str(),
                    "started_at": run.started_at,
                    "finished_at": run.finished_at,
                }
            })
        } else {
            serde_json::json!({
                "binary": binary,
                "ritual": ritual,
                "path": run_root.display().to_string(),
                "spec": spec_path.display().to_string(),
                "report": report_path.display().to_string(),
                "metadata": disk_metadata,
            })
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("Ritual run");
    println!("  Binary: {}", binary);
    println!("  Ritual: {}", ritual);
    println!("  Path:   {}", run_root.display());
    println!("  Spec:   {}", spec_path.display());
    println!("  Report: {}", report_path.display());
    match (db_run, disk_metadata) {
        (Some(run), _) => {
            println!("  Status: {}", run.status.as_str());
            if let Some(bh) = run.binary_hash {
                println!("  Binary hash: {}", bh);
            }
            println!("  Spec hash: {}", run.spec_hash);
            println!("  Backend: {}", run.backend);
            println!("  Started:  {}", run.started_at);
            println!("  Finished: {}", run.finished_at);
        }
        (None, Some(meta)) => {
            println!("  Status: {}", meta.status.as_str());
            if let Some(bh) = meta.binary_hash {
                println!("  Binary hash: {}", bh);
            }
            println!("  Spec hash: {}", meta.spec_hash);
            println!("  Backend: {}", meta.backend);
            println!("  Started:  {}", meta.started_at);
            println!("  Finished: {}", meta.finished_at);
        }
        _ => println!("  (No run metadata found in DB or disk)"),
    }

    Ok(())
}

/// List ritual specs under rituals/ (yaml/yml/json).
pub fn list_ritual_specs_command(root: &str, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);
    let dir = &layout.rituals_dir;
    if !dir.exists() {
        println!("Rituals dir missing at {}", dir.display());
        return Ok(());
    }

    let specs = collect_ritual_specs(dir)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&specs)?);
        return Ok(());
    }

    if specs.is_empty() {
        println!("Ritual specs: (none)");
        return Ok(());
    }

    println!("Ritual specs:");
    for spec in specs {
        let binary_display = spec.binary.as_deref().unwrap_or("(unspecified)");
        println!(
            "- {} (binary: {}, format: {}, path: {})",
            spec.name, binary_display, spec.format, spec.path
        );
    }
    Ok(())
}

/// Update status of a ritual run in the DB.
pub fn update_ritual_run_status_command(
    root: &str,
    binary: &str,
    ritual: &str,
    status: &str,
    finished_at: Option<String>,
) -> Result<()> {
    let status_enum = validate_run_status(status)?;

    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    let (_config, _db_path, db) = open_project_db(&layout)?;
    let finished_val = finished_at.unwrap_or_else(|| Utc::now().to_rfc3339());

    let updated = db
        .update_ritual_run_status(binary, ritual, status_enum.as_str(), Some(&finished_val))
        .context("Failed to update ritual run status")?;
    if updated == 0 {
        return Err(anyhow!("No ritual run found for binary '{}' and ritual '{}'", binary, ritual));
    }

    println!(
        "Updated ritual run status: binary='{}' ritual='{}' status='{}' finished_at='{}'",
        binary,
        ritual,
        status_enum.as_str(),
        finished_val
    );

    Ok(())
}
