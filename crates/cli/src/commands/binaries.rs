use std::path::Path;

use crate::commands::open_project_db;
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

    let (_config, db_path, db) = open_project_db(&layout)?;

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

    let (_config, _db_path, db) = open_project_db(&layout)?;
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
