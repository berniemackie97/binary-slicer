use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::rituals::analysis_summary;
use crate::commands::{RitualRunInfo, RitualRunMetadata, RitualSpecInfo};

/// Load the project config JSON from disk (delegates to core helper).
pub fn load_project_config(
    layout: &ritual_core::db::ProjectLayout,
) -> Result<ritual_core::db::ProjectConfig> {
    ritual_core::db::load_project_config(layout)
}

/// Resolve the DB path (respecting relative/absolute config) and open a ProjectDb (delegates to core helper).
pub fn open_project_db(
    layout: &ritual_core::db::ProjectLayout,
) -> Result<(ritual_core::db::ProjectConfig, std::path::PathBuf, ritual_core::db::ProjectDb)> {
    ritual_core::db::open_project_db(layout)
}

/// Helper to print whether a directory exists.
pub fn print_dir_status(label: &str, path: &Path) {
    let exists = path.is_dir();
    println!("- {label}: {} ({})", if exists { "OK" } else { "MISSING" }, path.display());
}

/// Load runs from DB (if available); does not merge disk runs.
pub fn load_runs_from_db(
    layout: &ritual_core::db::ProjectLayout,
    binary_filter: Option<&str>,
) -> Result<Vec<ritual_core::db::RitualRunRecord>> {
    let (_config, _db_path, db) = open_project_db(layout)?;
    Ok(db.list_ritual_runs(binary_filter).unwrap_or_default())
}

/// Discover ritual runs by scanning outputs/binaries/<binary>/<ritual>/.
pub fn collect_ritual_runs_on_disk(
    layout: &ritual_core::db::ProjectLayout,
    binary_filter: Option<&str>,
) -> Result<Vec<RitualRunInfo>> {
    let mut runs = Vec::new();
    if !layout.outputs_binaries_dir.exists() {
        return Ok(runs);
    }

    for bin_entry in fs::read_dir(&layout.outputs_binaries_dir)
        .with_context(|| format!("Failed to read {}", layout.outputs_binaries_dir.display()))?
    {
        let bin_entry = bin_entry?;
        if !bin_entry.file_type()?.is_dir() {
            continue;
        }
        let bin_name = bin_entry.file_name().to_string_lossy().to_string();
        if let Some(filter) = binary_filter {
            if bin_name != filter {
                continue;
            }
        }
        let bin_path = bin_entry.path();
        for run_entry in fs::read_dir(&bin_path)
            .with_context(|| format!("Failed to read {}", bin_path.display()))?
        {
            let run_entry = run_entry?;
            if !run_entry.file_type()?.is_dir() {
                continue;
            }
            let run_name = run_entry.file_name().to_string_lossy().to_string();
            let run_path = run_entry.path();
            let metadata_path = run_path.join("run_metadata.json");
            let (
                started_at,
                finished_at,
                status,
                spec_hash,
                backend,
                backend_version,
                backend_path,
            ) = if metadata_path.exists() {
                match fs::read_to_string(&metadata_path)
                    .ok()
                    .and_then(|body| serde_json::from_str::<RitualRunMetadata>(&body).ok())
                {
                    Some(meta) => (
                        Some(meta.started_at),
                        Some(meta.finished_at),
                        Some(meta.status.as_str().to_string()),
                        Some(meta.spec_hash),
                        Some(meta.backend),
                        meta.backend_version,
                        meta.backend_path,
                    ),
                    None => (None, None, None, None, None, None, None),
                }
            } else {
                (None, None, None, None, None, None, None)
            };

            runs.push(RitualRunInfo {
                binary: bin_name.clone(),
                name: run_name,
                path: run_path.display().to_string(),
                started_at,
                finished_at,
                status,
                spec_hash,
                backend,
                backend_version,
                backend_path,
                analysis: None,
            });
        }
    }
    Ok(runs)
}

/// Discover ritual specs under rituals/ (yaml/yml/json).
pub fn collect_ritual_specs(dir: &Path) -> Result<Vec<RitualSpecInfo>> {
    let mut specs = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let entry_path = entry.path();
        let ext_owned =
            entry_path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_string();
        let ext = ext_owned.as_str();
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        let format = ext.to_string();
        let body = fs::read_to_string(&entry_path)
            .with_context(|| format!("Failed to read ritual spec {}", entry_path.display()))?;
        let (name_field, binary_field) = if format == "json" {
            let parsed: Option<serde_json::Value> = serde_json::from_str(&body).ok();
            let name = parsed
                .as_ref()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string());
            let binary = parsed
                .as_ref()
                .and_then(|v| v.get("binary").and_then(|b| b.as_str()))
                .map(|s| s.to_string());
            (name, binary)
        } else {
            let parsed: Option<serde_yaml::Value> = serde_yaml::from_str(&body).ok();
            let name = parsed
                .as_ref()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string());
            let binary = parsed
                .as_ref()
                .and_then(|v| v.get("binary").and_then(|b| b.as_str()))
                .map(|s| s.to_string());
            (name, binary)
        };
        let name = name_field.unwrap_or_else(|| {
            entry_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string()
        });

        specs.push(RitualSpecInfo {
            name,
            binary: binary_field,
            path: entry_path.display().to_string(),
            format,
        });
    }

    specs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(specs)
}

/// Merge runs from DB and disk (disk only used to backfill missing DB runs).
pub fn load_runs_from_db_and_disk(
    layout: &ritual_core::db::ProjectLayout,
    binary_filter: Option<&str>,
) -> Result<Vec<RitualRunInfo>> {
    let (_config, _db_path, db) = open_project_db(layout)?;
    let mut runs: Vec<RitualRunInfo> = Vec::new();
    let db_runs = db.list_ritual_runs(binary_filter).unwrap_or_default();
    for run in &db_runs {
        let mut info = crate::commands::db_run_to_info(layout, run);
        if let Ok(Some(analysis)) = db.load_analysis_result(&run.binary, &run.ritual) {
            info.analysis = Some(analysis_summary(&analysis, Some(run)));
        }
        runs.push(info);
    }

    // Merge in on-disk runs not in DB.
    let disk_runs = collect_ritual_runs_on_disk(layout, binary_filter)?;
    for dr in disk_runs {
        if !runs.iter().any(|r| r.binary == dr.binary && r.name == dr.name) {
            runs.push(dr);
        }
    }
    runs.sort_by(|a, b| a.name.cmp(&b.name).then(a.binary.cmp(&b.binary)));
    Ok(runs)
}
