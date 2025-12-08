use std::fs;

use crate::canonicalize_or_current;
use anyhow::{Context, Result};

/// Initialize a new slice record and its documentation scaffold.
pub fn init_slice_command(root: &str, name: &str, description: Option<String>) -> Result<()> {
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
        .with_description(description.clone());
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
        let serialized = serde_json::to_string_pretty(&slices)?;
        println!("{}", serialized);
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
        println!("- {} ({:?}) - {}", slice.name, slice.status, desc);
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
        contents.push_str(
            "## Roots\n- TODO: list root functions (by address/name) that define this slice.\n\n",
        );
        contents.push_str("## Functions\n- TODO: populated by analysis runs.\n\n");
        contents.push_str("## Evidence\n- TODO: xrefs, strings, patterns that justify membership in this slice.\n");

        fs::write(&doc_path, contents)
            .with_context(|| format!("Failed to write slice doc at {}", doc_path.display()))?;
        println!("Emitted slice doc: {}", doc_path.display());
    }

    Ok(())
}

/// Regenerate slice reports for all slices in the DB.
pub fn emit_slice_reports_command(root: &str) -> Result<()> {
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

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    let slices = db.list_slices().context("Failed to list slices")?;
    if slices.is_empty() {
        println!("No slices to emit reports for.");
        return Ok(());
    }

    for slice in slices {
        let report_path = layout.reports_dir.join(format!("{}.json", slice.name));
        let report = serde_json::json!({
            "name": slice.name,
            "description": slice.description,
            "status": format!("{:?}", slice.status),
            "roots": [],
            "functions": [],
            "evidence": [],
        });
        let serialized = serde_json::to_string_pretty(&report)?;
        fs::write(&report_path, serialized).with_context(|| {
            format!("Failed to write slice report at {}", report_path.display())
        })?;
        println!("Emitted slice report: {}", report_path.display());
    }

    Ok(())
}
