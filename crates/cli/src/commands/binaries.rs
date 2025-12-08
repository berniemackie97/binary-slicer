use std::fs;
use std::path::Path;

use crate::{canonicalize_or_current, sha256_file};
use anyhow::{anyhow, Context, Result};

/// Register a binary in the project database.
pub fn add_binary_command(
    root: &str,
    path: &str,
    name: Option<String>,
    arch: Option<String>,
    hash: Option<String>,
    skip_hash: bool,
) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Load project config so we know where the DB lives.
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

    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    // Normalize the binary path.
    let input_path = Path::new(path);
    let abs_path = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        root_path.join(input_path)
    };

    if !abs_path.exists() {
        return Err(anyhow!("Binary file does not exist: {}", abs_path.display()));
    }

    // Store path relative to project root when possible.
    let rel_path = abs_path
        .canonicalize()
        .ok()
        .and_then(|abs_canon| {
            root_path.canonicalize().ok().and_then(|root_canon| {
                abs_canon.strip_prefix(&root_canon).ok().map(|p| p.to_path_buf())
            })
        })
        .or_else(|| abs_path.strip_prefix(&root_path).ok().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| abs_path.clone());
    let rel_path_str = rel_path.to_string_lossy().to_string();

    let binary_name = name.unwrap_or_else(|| {
        input_path.file_name().and_then(|os| os.to_str()).unwrap_or(path).to_string()
    });

    let hash = if let Some(h) = hash {
        Some(h)
    } else if skip_hash {
        None
    } else {
        Some(sha256_file(&abs_path)?)
    };

    let record =
        ritual_core::db::BinaryRecord { name: binary_name, path: rel_path_str, arch, hash };

    let id = db.insert_binary(&record).context("Failed to insert binary record")?;

    println!("Added binary:");
    println!("  Id: {}", id);
    println!("  Name: {}", record.name);
    println!("  Path (relative): {}", record.path);
    println!("  DB: {}", db_path.display());

    Ok(())
}

/// List all binaries registered in the project database.
pub fn list_binaries_command(root: &str, json: bool) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Load project config so we know where the DB lives.
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

    if json {
        let serialized = serde_json::to_string_pretty(&binaries)?;
        println!("{}", serialized);
        return Ok(());
    }

    if binaries.is_empty() {
        println!("Binaries:");
        println!("(none)");
        return Ok(());
    }

    println!("Binaries:");
    for bin in binaries {
        let arch_display = bin.arch.as_deref().unwrap_or("(unspecified)");
        let hash_display = bin.hash.as_deref().unwrap_or("(none)");
        println!(
            "- {} (path: {}, arch: {}, hash: {})",
            bin.name, bin.path, arch_display, hash_display
        );
    }

    Ok(())
}
