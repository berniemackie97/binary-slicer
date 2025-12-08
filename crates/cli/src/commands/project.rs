use std::fs;
use std::path::Path;

use crate::{canonicalize_or_current, infer_project_name};
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Serialize)]
pub struct ProjectInfoSnapshot {
    pub name: String,
    pub root: String,
    pub config_file: String,
    pub config_version: String,
    pub db_path: String,
    pub layout: ProjectInfoLayout,
    pub binaries: Vec<ritual_core::db::BinaryRecord>,
    pub slices: Vec<ritual_core::db::SliceRecord>,
    pub ritual_runs: Vec<crate::commands::RitualRunInfo>,
    pub ritual_specs: Vec<crate::commands::RitualSpecInfo>,
}

#[derive(Serialize)]
pub struct ProjectInfoLayout {
    pub meta_dir: String,
    pub docs_dir: String,
    pub slices_docs_dir: String,
    pub reports_dir: String,
    pub graphs_dir: String,
    pub rituals_dir: String,
    pub outputs_dir: String,
}

/// Initialize a new project at `root`.
pub fn init_project_command(root: &str, name: Option<String>) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Derive project name if not provided.
    let project_name = match name {
        Some(n) => n,
        None => infer_project_name(&root_path),
    };

    // Ensure directories exist.
    fs::create_dir_all(&layout.meta_dir)
        .with_context(|| format!("Failed to create meta dir: {}", layout.meta_dir.display()))?;
    fs::create_dir_all(&layout.slices_docs_dir).with_context(|| {
        format!("Failed to create slices docs dir: {}", layout.slices_docs_dir.display())
    })?;
    fs::create_dir_all(&layout.reports_dir).with_context(|| {
        format!("Failed to create reports dir: {}", layout.reports_dir.display())
    })?;
    fs::create_dir_all(&layout.graphs_dir)
        .with_context(|| format!("Failed to create graphs dir: {}", layout.graphs_dir.display()))?;
    fs::create_dir_all(&layout.rituals_dir).with_context(|| {
        format!("Failed to create rituals dir: {}", layout.rituals_dir.display())
    })?;
    fs::create_dir_all(&layout.outputs_dir).with_context(|| {
        format!("Failed to create outputs dir: {}", layout.outputs_dir.display())
    })?;
    fs::create_dir_all(&layout.outputs_binaries_dir).with_context(|| {
        format!(
            "Failed to create per-binary outputs dir: {}",
            layout.outputs_binaries_dir.display()
        )
    })?;

    // Build project config.
    let db_path_rel = layout.db_path_relative_string();
    let config = ritual_core::db::ProjectConfig::new(&project_name, db_path_rel);

    // Serialize and write config JSON.
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&layout.project_config_path, json).with_context(|| {
        format!("Failed to write project config: {}", layout.project_config_path.display())
    })?;

    // Create the project database immediately so follow-on commands (and tests)
    // can rely on its presence.
    ritual_core::db::ProjectDb::open(&layout.db_path).with_context(|| {
        format!("Failed to initialize project database at {}", layout.db_path.display())
    })?;

    println!("Initialized Binary Slicer project:");
    println!("  Name: {}", project_name);
    println!("  Root: {}", layout.root.display());
    println!("  Config: {}", layout.project_config_path.display());
    println!("  DB path (relative): {}", config.db.path);
    println!("  Docs dir: {}", layout.docs_dir.display());
    println!("  Slices docs dir: {}", layout.slices_docs_dir.display());
    println!("  Reports dir: {}", layout.reports_dir.display());
    println!("  Graphs dir: {}", layout.graphs_dir.display());
    println!("  Rituals dir: {}", layout.rituals_dir.display());
    println!("  Outputs dir: {}", layout.outputs_dir.display());

    Ok(())
}

/// Show basic information about an existing project.
pub fn project_info_command(root: &str, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Read the project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;

    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    // Load DB metadata.
    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;
    let binaries = db.list_binaries().context("Failed to list binaries")?;
    let slices = db.list_slices().context("Failed to list slices")?;
    let db_runs = db.list_ritual_runs(None).unwrap_or_default();

    let mut ritual_runs: Vec<crate::commands::RitualRunInfo> =
        db_runs.iter().map(|r| crate::commands::db_run_to_info(&layout, r)).collect();

    // Include any on-disk runs not yet in the DB (backward compatibility).
    let disk_runs = crate::commands::collect_ritual_runs_on_disk(&layout, None)?;
    for dr in disk_runs {
        if !ritual_runs.iter().any(|r| r.binary == dr.binary && r.name == dr.name) {
            ritual_runs.push(dr);
        }
    }
    ritual_runs.sort_by(|a, b| a.name.cmp(&b.name).then(a.binary.cmp(&b.binary)));
    let ritual_specs =
        crate::commands::collect_ritual_specs(&layout.rituals_dir).unwrap_or_default();

    if json {
        let snapshot = ProjectInfoSnapshot {
            name: config.name.clone(),
            root: layout.root.display().to_string(),
            config_file: layout.project_config_path.display().to_string(),
            config_version: config.config_version.clone(),
            db_path: config.db.path.clone(),
            layout: ProjectInfoLayout {
                meta_dir: layout.meta_dir.display().to_string(),
                docs_dir: layout.docs_dir.display().to_string(),
                slices_docs_dir: layout.slices_docs_dir.display().to_string(),
                reports_dir: layout.reports_dir.display().to_string(),
                graphs_dir: layout.graphs_dir.display().to_string(),
                rituals_dir: layout.rituals_dir.display().to_string(),
                outputs_dir: layout.outputs_dir.display().to_string(),
            },
            binaries,
            slices,
            ritual_runs,
            ritual_specs,
        };
        let serialized = serde_json::to_string_pretty(&snapshot)?;
        println!("{}", serialized);
        return Ok(());
    }

    println!("Binary Slicer Project Info");
    println!("==========================");
    println!("Name: {}", config.name);
    println!("Root: {}", layout.root.display());
    println!("Config file: {}", layout.project_config_path.display());
    println!("Config version: {}", config.config_version);
    println!("DB path (config): {}", config.db.path);
    println!();

    // Basic directory existence checks.
    println!("Directories:");
    crate::commands::print_dir_status("Meta dir (.ritual)", &layout.meta_dir);
    crate::commands::print_dir_status("Docs dir", &layout.docs_dir);
    crate::commands::print_dir_status("Slices docs dir", &layout.slices_docs_dir);
    crate::commands::print_dir_status("Reports dir", &layout.reports_dir);
    crate::commands::print_dir_status("Graphs dir", &layout.graphs_dir);
    crate::commands::print_dir_status("Rituals dir", &layout.rituals_dir);
    crate::commands::print_dir_status("Outputs dir", &layout.outputs_dir);
    println!();
    println!("Ritual specs: {}", ritual_specs.len());
    println!("Ritual runs: {}", ritual_runs.len());

    Ok(())
}
