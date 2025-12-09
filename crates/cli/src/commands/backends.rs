use anyhow::Result;
use serde::Serialize;

use ritual_core::db::BackendPaths;
use ritual_core::services::analysis::default_backend_registry;

#[derive(Debug, Serialize)]
pub struct BackendInfo {
    pub name: String,
    pub description: String,
}

/// List available analysis backends known to this binary.
pub fn list_backends_command(json: bool) -> Result<()> {
    let registry = default_backend_registry();
    let mut entries: Vec<BackendInfo> = registry
        .names()
        .into_iter()
        .map(|name| {
            let description = match name.as_str() {
                "validate-only" => {
                    "Checks binary existence; placeholder until real analyzers are configured"
                        .to_string()
                }
                "capstone" => "Capstone-based quick disassembly (x86_64 demo)".to_string(),
                "ghidra" => {
                    "Ghidra headless stub (requires GHIDRA_ANALYZE_HEADLESS or GHIDRA_INSTALL_DIR)"
                        .to_string()
                }
                other => format!("Backend '{}'", other),
            };
            BackendInfo { name: name.clone(), description }
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!("Backends: (none)");
        return Ok(());
    }

    println!("Backends:");
    for entry in entries {
        println!("- {}: {}", entry.name, entry.description);
    }

    Ok(())
}

/// Best-effort detection of configured backend tool paths from project config.
pub fn configured_backend_paths(config: &ritual_core::db::ProjectConfig) -> BackendPaths {
    config.backends.clone()
}
