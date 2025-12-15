use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::canonicalize_or_current;
use crate::commands::load_project_config;

/// Interactive setup for analysis backends (rizin, ghidra).
pub fn setup_backend_command(
    root: &str,
    backend: &str,
    tool_path: Option<String>,
    set_default: bool,
    write_path: bool,
) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);
    let mut config = load_project_config(&layout)?;

    match backend {
        "rizin" => {
            let resolved = tool_path.map(PathBuf::from).or_else(find_rizin).ok_or_else(|| {
                anyhow!("Could not find rizin. Install it and/or pass --path <rizin>")
            })?;
            validate_executable(&resolved, "rizin")?;
            println!("Found rizin at {}", resolved.display());
            config.backends.rizin = Some(resolved.to_string_lossy().to_string());
            config.backend_versions.rizin = detect_rizin_version(&resolved);
            maybe_update_path(write_path, resolved.parent());
        }
        "ghidra" => {
            let resolved = tool_path
                .map(PathBuf::from)
                .or_else(resolve_ghidra_headless)
                .ok_or_else(|| anyhow!("Could not find analyzeHeadless. Set GHIDRA_ANALYZE_HEADLESS, GHIDRA_INSTALL_DIR, or pass --path"))?;
            validate_executable(&resolved, "analyzeHeadless")?;
            println!("Found analyzeHeadless at {}", resolved.display());
            config.backends.ghidra_headless = Some(resolved.to_string_lossy().to_string());
            config.backend_versions.ghidra_headless = detect_ghidra_version(&resolved);
            maybe_update_path(write_path, resolved.parent());
        }
        other => return Err(anyhow!("Unsupported backend '{}'", other)),
    }

    if set_default {
        config.default_backend = Some(backend.to_string());
        println!("Set default_backend to {}", backend);
    }

    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&layout.project_config_path, json)
        .with_context(|| format!("Failed to write {}", layout.project_config_path.display()))?;
    println!("Updated project config at {}", layout.project_config_path.display());

    Ok(())
}

fn validate_executable(path: &Path, name: &str) -> Result<()> {
    if !path.is_file() {
        return Err(anyhow!("{} not found at {}", name, path.display()));
    }
    Ok(())
}

fn find_rizin() -> Option<PathBuf> {
    find_in_path(if cfg!(windows) { "rizin.exe" } else { "rizin" })
}

fn resolve_ghidra_headless() -> Option<PathBuf> {
    if let Ok(p) = env::var("GHIDRA_ANALYZE_HEADLESS") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(dir) = env::var("GHIDRA_INSTALL_DIR") {
        let mut pb = PathBuf::from(dir);
        pb = pb.join(if cfg!(windows) { "analyzeHeadless.bat" } else { "analyzeHeadless" });
        if pb.is_file() {
            return Some(pb);
        }
    }
    None
}

fn find_in_path(executable: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|p| {
            let candidate = p.join(executable);
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

fn maybe_update_path(write_path: bool, dir: Option<&Path>) {
    if dir.is_none() {
        return;
    }
    let dir = dir.unwrap();
    println!(
        "To use now, add to PATH for this session:\n  {}",
        if cfg!(windows) {
            format!(r#"$env:PATH = "{};{}""#, dir.display(), env::var("PATH").unwrap_or_default())
        } else {
            format!(r#"export PATH="{}:$PATH""#, dir.display())
        }
    );

    if write_path {
        if let Err(e) = append_to_profile(dir) {
            eprintln!("Failed to append to shell profile: {}", e);
        } else {
            println!("Appended PATH export to shell profile.");
        }
    } else {
        println!("(Use --write-path to append this to your shell profile automatically.)");
    }
}

fn append_to_profile(dir: &Path) -> Result<()> {
    if cfg!(windows) {
        let profile = env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Documents")
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1");
        if let Some(parent) = profile.parent() {
            fs::create_dir_all(parent).ok();
        }
        let line =
            format!(r#"$env:PATH = "{};{}""#, dir.display(), env::var("PATH").unwrap_or_default());
        append_line(&profile, &line)
    } else {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let candidates = [".bashrc", ".zshrc", ".profile"];
        let target = candidates
            .iter()
            .map(|f| PathBuf::from(&home).join(f))
            .find(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from(&home).join(".bashrc"));
        let line = format!(r#"export PATH="{}:$PATH""#, dir.display());
        append_line(&target, &line)
    }
}

fn append_line(path: &Path, line: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

fn detect_rizin_version(path: &Path) -> Option<String> {
    Command::new(path).arg("-v").output().ok().and_then(|out| {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        }
    })
}

fn detect_ghidra_version(path: &Path) -> Option<String> {
    Command::new(path).arg("-version").output().ok().and_then(|out| {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        }
    })
}
