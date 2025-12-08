use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use binary_slicer::{canonicalize_or_current, infer_project_name, sha256_file};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;

/// Slice-oriented reverse-engineering assistant CLI.
///
/// This CLI is a thin wrapper around `ritual-core` (exposed in code as `ritual_core`).
/// All substantive logic lives in the library so it can be tested thoroughly
/// and reused from other frontends.
#[derive(Parser, Debug)]
#[command(
    name = "binary-slicer",
    version,
    about = "Slice-oriented reverse-engineering assistant",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Simple smoke-test command to verify the toolchain and wiring.
    Hello {
        /// Optional slice name to greet.
        #[arg(long, default_value = "DefaultSlice")]
        slice: String,
    },

    /// Initialize a new Binary Slicer project at the given root.
    ///
    /// This will:
    /// - Create a `.ritual` metadata directory.
    /// - Create `docs/slices`, `reports`, and `graphs` directories.
    /// - Write a `.ritual/project.json` config file.
    InitProject {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Optional project name. If omitted, the name is derived from the root directory.
        #[arg(long)]
        name: Option<String>,
    },

    /// Show basic information about an existing Binary Slicer project.
    ///
    /// This reads `.ritual/project.json` and reports key paths and config values.
    ProjectInfo {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Register a binary (e.g., libExampleGame.so) in the project database.
    ///
    /// This does not perform analysis; it just records that the binary exists
    /// and where it lives relative to the project root.
    AddBinary {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Path to the binary to register.
        #[arg(long)]
        path: String,

        /// Optional human-friendly name. Defaults to the file name.
        #[arg(long)]
        name: Option<String>,

        /// Optional architecture hint (e.g., armv7, x86_64).
        #[arg(long)]
        arch: Option<String>,

        /// Optional precomputed hash. If omitted, the CLI computes SHA-256 unless `--skip-hash` is set.
        #[arg(long)]
        hash: Option<String>,

        /// Skip hash computation (stores no hash).
        #[arg(long, default_value_t = false)]
        skip_hash: bool,
    },

    /// Initialize a new slice record and its documentation scaffold.
    ///
    /// This will:
    /// - Insert a slice record into the project database.
    /// - Create `docs/slices/<Name>.md` with a basic template.
    InitSlice {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Name of the slice (e.g., `AutoUpdateManager`).
        #[arg(long)]
        name: String,

        /// Optional human-readable description of the slice.
        #[arg(long)]
        description: Option<String>,
    },

    /// List all slices registered in the project database.
    ///
    /// Shows name, status, and optional description for each slice.
    ListSlices {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// List all binaries registered in the project database.
    ///
    /// Shows name, path, arch, and hash if available.
    ListBinaries {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Regenerate slice docs for all slices registered in the project DB.
    EmitSliceDocs {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,
    },

    /// Regenerate slice JSON reports for all slices registered in the project DB.
    EmitSliceReports {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,
    },

    /// Run a ritual spec (YAML/JSON) against a target binary (analysis stub for now).
    RunRitual {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Path to the ritual spec (YAML/JSON).
        #[arg(long)]
        file: String,

        /// Overwrite an existing ritual run output directory if present.
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Clean ritual outputs under `outputs/binaries` with safety guardrails.
    CleanOutputs {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Binary name to clean (required unless --all is set).
        #[arg(long)]
        binary: Option<String>,

        /// Specific ritual run to clean (requires --binary).
        #[arg(long)]
        ritual: Option<String>,

        /// Clean all outputs/binaries (dangerous; requires --yes).
        #[arg(long, default_value_t = false)]
        all: bool,

        /// Required confirmation flag to avoid accidental deletion.
        #[arg(long, default_value_t = false)]
        yes: bool,
    },

    /// List ritual runs discovered under outputs/binaries (human or JSON).
    ListRitualRuns {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Optional binary name to filter runs.
        #[arg(long)]
        binary: Option<String>,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Show details for a ritual run (metadata + paths).
    ShowRitualRun {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Binary name (required).
        #[arg(long)]
        binary: String,

        /// Ritual name (required).
        #[arg(long)]
        ritual: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// List ritual specs discovered under `rituals/` (human or JSON).
    ListRitualSpecs {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Update the status of a ritual run recorded in the project DB.
    UpdateRitualRunStatus {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Binary name (required).
        #[arg(long)]
        binary: String,

        /// Ritual name (required).
        #[arg(long)]
        ritual: String,

        /// New status (one of: pending, running, succeeded, failed, canceled, stubbed).
        #[arg(long)]
        status: String,

        /// Optional finished_at timestamp override (RFC3339). Defaults to now if omitted.
        #[arg(long)]
        finished_at: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Default to the Hello command if none is provided.
    match cli.command.unwrap_or(Command::Hello { slice: "DefaultSlice".to_string() }) {
        Command::Hello { slice } => hello_command(&slice)?,
        Command::InitProject { root, name } => init_project_command(&root, name)?,
        Command::ProjectInfo { root, json } => project_info_command(&root, json)?,
        Command::AddBinary { root, path, name, arch, hash, skip_hash } => {
            add_binary_command(&root, &path, name, arch, hash, skip_hash)?
        }
        Command::InitSlice { root, name, description } => {
            init_slice_command(&root, &name, description)?
        }
        Command::ListSlices { root, json } => list_slices_command(&root, json)?,
        Command::ListBinaries { root, json } => list_binaries_command(&root, json)?,
        Command::EmitSliceDocs { root } => emit_slice_docs_command(&root)?,
        Command::EmitSliceReports { root } => emit_slice_reports_command(&root)?,
        Command::RunRitual { root, file, force } => run_ritual_command(&root, &file, force)?,
        Command::CleanOutputs { root, binary, ritual, all, yes } => {
            clean_outputs_command(&root, binary.as_deref(), ritual.as_deref(), all, yes)?
        }
        Command::ListRitualRuns { root, binary, json } => {
            list_ritual_runs_command(&root, binary.as_deref(), json)?
        }
        Command::ShowRitualRun { root, binary, ritual, json } => {
            show_ritual_run_command(&root, &binary, &ritual, json)?
        }
        Command::ListRitualSpecs { root, json } => list_ritual_specs_command(&root, json)?,
        Command::UpdateRitualRunStatus { root, binary, ritual, status, finished_at } => {
            update_ritual_run_status_command(&root, &binary, &ritual, &status, finished_at)?
        }
    }

    Ok(())
}

/// "Hello" smoke-test command.
fn hello_command(slice_name: &str) -> Result<()> {
    // NOTE: crate name `ritual-core` in Cargo.toml is `ritual_core` in Rust code.
    let version = ritual_core::version();
    let result = ritual_core::analysis::hello_slice(slice_name);

    println!("binary-slicer v{}", version);
    println!("Hello, slice: {}", result.slice.name);
    println!("Functions in this (stub) slice:");
    for func in result.functions {
        println!("  - {}", func.name);
    }

    Ok(())
}

/// Initialize a new project at `root`.
fn init_project_command(root: &str, name: Option<String>) -> Result<()> {
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
    println!("  Outputs dir: {}", layout.outputs_dir.display());

    Ok(())
}

#[derive(Serialize)]
struct ProjectInfoSnapshot {
    name: String,
    root: String,
    config_file: String,
    config_version: String,
    db_path: String,
    layout: ProjectInfoLayout,
    binaries: Vec<ritual_core::db::BinaryRecord>,
    slices: Vec<ritual_core::db::SliceRecord>,
    ritual_runs: Vec<RitualRunInfo>,
    ritual_specs: Vec<RitualSpecInfo>,
}

#[derive(Serialize)]
struct ProjectInfoLayout {
    meta_dir: String,
    docs_dir: String,
    slices_docs_dir: String,
    reports_dir: String,
    graphs_dir: String,
    rituals_dir: String,
    outputs_dir: String,
}

#[derive(Debug, Serialize, Clone)]
struct RitualRunInfo {
    binary: String,
    name: String,
    path: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    status: Option<String>,
    spec_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RitualSpecInfo {
    name: String,
    binary: Option<String>,
    path: String,
    format: String,
}

/// Show basic information about an existing project.
fn project_info_command(root: &str, json: bool) -> Result<()> {
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
    let mut ritual_runs: Vec<RitualRunInfo> =
        db_runs.iter().map(|r| db_run_to_info(&layout, r)).collect();
    // Include any on-disk runs not yet in the DB (backward compatibility).
    let disk_runs = collect_ritual_runs_on_disk(&layout, None)?;
    for dr in disk_runs {
        if !ritual_runs.iter().any(|r| r.binary == dr.binary && r.name == dr.name) {
            ritual_runs.push(dr);
        }
    }
    ritual_runs.sort_by(|a, b| a.name.cmp(&b.name).then(a.binary.cmp(&b.binary)));
    let ritual_specs = collect_ritual_specs(&layout.rituals_dir).unwrap_or_default();

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
    print_dir_status("Meta dir (.ritual)", &layout.meta_dir);
    print_dir_status("Docs dir", &layout.docs_dir);
    print_dir_status("Slices docs dir", &layout.slices_docs_dir);
    print_dir_status("Reports dir", &layout.reports_dir);
    print_dir_status("Graphs dir", &layout.graphs_dir);
    print_dir_status("Rituals dir", &layout.rituals_dir);
    print_dir_status("Outputs dir", &layout.outputs_dir);
    println!();
    println!("Ritual specs: {}", ritual_specs.len());
    println!("Ritual runs: {}", ritual_runs.len());

    Ok(())
}

/// Register a binary in the project database.
fn add_binary_command(
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

/// Initialize a slice record and its documentation scaffold.
fn init_slice_command(root: &str, name: &str, description: Option<String>) -> Result<()> {
    use ritual_core::db::{ProjectConfig, ProjectDb, ProjectLayout, SliceRecord, SliceStatus};

    let root_path = canonicalize_or_current(root)?;
    let layout = ProjectLayout::new(&root_path);

    // Load project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    // Resolve DB path (may be relative or absolute in config).
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    // Insert or update the slice record.
    let record = SliceRecord::new(name, SliceStatus::Planned).with_description(description.clone());
    db.insert_slice(&record).context("Failed to insert slice record")?;

    // Create/update the slice doc in docs/slices/<Name>.md.
    let doc_path = layout.slices_docs_dir.join(format!("{name}.md"));

    if !doc_path.exists() {
        let mut contents = String::new();
        contents.push_str(&format!("# {name}\n\n"));
        if let Some(desc) = &description {
            contents.push_str(desc);
            contents.push_str("\n\n");
        } else {
            contents.push_str("TODO: add a human-readable description of this slice.\n\n");
        }
        contents.push_str("## Roots\n");
        contents
            .push_str("- TODO: list root functions (by address/name) that define this slice.\n\n");
        contents.push_str("## Functions\n");
        contents.push_str("- TODO: populated by analysis runs.\n\n");
        contents.push_str("## Evidence\n");
        contents
            .push_str("- TODO: xrefs, strings, patterns that justify membership in this slice.\n");

        fs::write(&doc_path, contents)
            .with_context(|| format!("Failed to write slice doc at {}", doc_path.display()))?;
    }

    println!("Initialized slice:");
    println!("  Name: {name}");
    println!("  Root: {}", layout.root.display());
    println!("  Doc:  {}", doc_path.display());

    Ok(())
}

/// List all slices registered in the project database.
fn list_slices_command(root: &str, json: bool) -> Result<()> {
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
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    let slices = db.list_slices().context("Failed to list slices")?;

    if json {
        let serialized =
            serde_json::to_string_pretty(&slices).context("Failed to serialize slices to JSON")?;
        println!("{}", serialized);
    } else {
        println!("Slices ({}):", slices.len());
        if slices.is_empty() {
            println!("  (none)");
            return Ok(());
        }

        for slice in slices {
            let status_str = format!("{:?}", slice.status);
            match slice.description {
                Some(desc) => {
                    println!("  - {} [{}] - {}", slice.name, status_str, desc);
                }
                None => {
                    println!("  - {} [{}]", slice.name, status_str);
                }
            }
        }
    }

    Ok(())
}

/// List all binaries registered in the project database.
fn list_binaries_command(root: &str, json: bool) -> Result<()> {
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
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    let binaries = db.list_binaries().context("Failed to list binaries")?;

    if json {
        let serialized = serde_json::to_string_pretty(&binaries)
            .context("Failed to serialize binaries to JSON")?;
        println!("{}", serialized);
    } else {
        println!("Binaries ({}):", binaries.len());
        if binaries.is_empty() {
            println!("  (none)");
            return Ok(());
        }

        for bin in binaries {
            let arch_display = bin.arch.as_deref().unwrap_or("-");
            let hash_display = bin.hash.as_deref().unwrap_or("-");
            println!(
                "  - {} [arch: {}] path={} hash={}",
                bin.name, arch_display, bin.path, hash_display
            );
        }
    }

    Ok(())
}

/// Regenerate slice docs for all slices in the DB.
fn emit_slice_docs_command(root: &str) -> Result<()> {
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
    let config_db_path = Path::new(&config.db.path);
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
fn emit_slice_reports_command(root: &str) -> Result<()> {
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
    let config_db_path = Path::new(&config.db.path);
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

/// Run a ritual spec (stub analysis) and organize outputs per binary and ritual name.
fn run_ritual_command(root: &str, file: &str, force: bool) -> Result<()> {
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
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };
    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

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

    // Persist a normalized spec copy into the run directory.
    let normalized_spec_path = run_output_root.join("spec.yaml");
    let mut spec_copy = spec;
    if spec_copy.outputs.is_none() {
        spec_copy.outputs = Some(RitualOutputs { reports: true, graphs: true, docs: true });
    }
    let yaml = serde_yaml::to_string(&spec_copy).context("Failed to serialize ritual spec")?;
    fs::write(&normalized_spec_path, yaml).with_context(|| {
        format!("Failed to write normalized spec to {}", normalized_spec_path.display())
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
        Some(sha256_file(&binary_path)?)
    } else {
        None
    };

    // Stub analysis output placeholders.
    let report_path = run_output_root.join("report.json");
    let report = serde_json::json!({
        "ritual": spec_copy.name,
        "binary": target_bin.name,
        "roots": spec_copy.roots,
        "max_depth": spec_copy.max_depth,
        "status": "stubbed",
        "functions": [],
        "edges": [],
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
        started_at: now.clone(),
        finished_at: now,
        status: "stubbed".to_string(),
    };
    let metadata_path = run_output_root.join("run_metadata.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)
        .with_context(|| format!("Failed to write run metadata at {}", metadata_path.display()))?;

    // Persist run to the project DB.
    let run_record = ritual_core::db::RitualRunRecord {
        binary: metadata.binary.clone(),
        ritual: metadata.ritual.clone(),
        spec_hash: metadata.spec_hash.clone(),
        binary_hash: metadata.binary_hash.clone(),
        status: metadata.status.clone(),
        started_at: metadata.started_at.clone(),
        finished_at: metadata.finished_at.clone(),
    };
    db.insert_ritual_run(&run_record)
        .with_context(|| format!("Failed to record ritual run in DB {}", db_path.display()))?;

    println!("Ran ritual (stub): {}", spec_copy.name);
    println!("  Binary: {}", target_bin.name);
    println!("  Roots: {:?}", spec_copy.roots);
    println!("  Output: {}", run_output_root.display());

    Ok(())
}

/// Clean ritual outputs (per binary or per run) with confirmation gating.
fn clean_outputs_command(
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

/// List ritual runs discovered under outputs/binaries.
fn list_ritual_runs_command(root: &str, binary_filter: Option<&str>, json: bool) -> Result<()> {
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
fn show_ritual_run_command(root: &str, binary: &str, ritual: &str, json: bool) -> Result<()> {
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
                    "status": run.status,
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
            println!("  Status: {}", run.status);
            if let Some(bh) = run.binary_hash {
                println!("  Binary hash: {}", bh);
            }
            println!("  Spec hash: {}", run.spec_hash);
            println!("  Started:  {}", run.started_at);
            println!("  Finished: {}", run.finished_at);
        }
        (None, Some(meta)) => {
            println!("  Status: {}", meta.status);
            if let Some(bh) = meta.binary_hash {
                println!("  Binary hash: {}", bh);
            }
            println!("  Spec hash: {}", meta.spec_hash);
            println!("  Started:  {}", meta.started_at);
            println!("  Finished: {}", meta.finished_at);
        }
        _ => println!("  (No run metadata found in DB or disk)"),
    }

    Ok(())
}

/// Update status of a ritual run in the DB.
fn update_ritual_run_status_command(
    root: &str,
    binary: &str,
    ritual: &str,
    status: &str,
    finished_at: Option<String>,
) -> Result<()> {
    validate_run_status(status)?;

    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;
    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };

    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;
    let finished_val = finished_at.unwrap_or_else(|| Utc::now().to_rfc3339());

    let updated = db
        .update_ritual_run_status(binary, ritual, status, Some(&finished_val))
        .context("Failed to update ritual run status")?;
    if updated == 0 {
        return Err(anyhow!("No ritual run found for binary '{}' and ritual '{}'", binary, ritual));
    }

    println!(
        "Updated ritual run status: binary='{}' ritual='{}' status='{}' finished_at='{}'",
        binary, ritual, status, finished_val
    );

    Ok(())
}

/// List ritual specs under rituals/ (yaml/yml/json).
fn list_ritual_specs_command(root: &str, json: bool) -> Result<()> {
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

fn db_run_to_info(
    layout: &ritual_core::db::ProjectLayout,
    rec: &ritual_core::db::RitualRunRecord,
) -> RitualRunInfo {
    RitualRunInfo {
        binary: rec.binary.clone(),
        name: rec.ritual.clone(),
        path: layout.binary_output_root(&rec.binary).join(&rec.ritual).display().to_string(),
        started_at: Some(rec.started_at.clone()),
        finished_at: Some(rec.finished_at.clone()),
        status: Some(rec.status.clone()),
        spec_hash: Some(rec.spec_hash.clone()),
    }
}

/// Discover ritual runs by scanning outputs/binaries/<binary>/<ritual>/.
fn collect_ritual_runs_on_disk(
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
            let (started_at, finished_at, status, spec_hash) = if metadata_path.exists() {
                match fs::read_to_string(&metadata_path)
                    .ok()
                    .and_then(|body| serde_json::from_str::<RitualRunMetadata>(&body).ok())
                {
                    Some(meta) => (
                        Some(meta.started_at),
                        Some(meta.finished_at),
                        Some(meta.status),
                        Some(meta.spec_hash),
                    ),
                    None => (None, None, None, None),
                }
            } else {
                (None, None, None, None)
            };

            runs.push(RitualRunInfo {
                binary: bin_name.clone(),
                name: run_name,
                path: run_path.display().to_string(),
                started_at,
                finished_at,
                status,
                spec_hash,
            });
        }
    }
    Ok(runs)
}

fn collect_ritual_specs(dir: &Path) -> Result<Vec<RitualSpecInfo>> {
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

fn load_runs_from_db(
    layout: &ritual_core::db::ProjectLayout,
    binary_filter: Option<&str>,
) -> Result<Vec<ritual_core::db::RitualRunRecord>> {
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    let config_db_path = Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };
    let db = ritual_core::db::ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;

    Ok(db.list_ritual_runs(binary_filter).unwrap_or_default())
}

fn load_runs_from_db_and_disk(
    layout: &ritual_core::db::ProjectLayout,
    binary_filter: Option<&str>,
) -> Result<Vec<RitualRunInfo>> {
    let mut runs: Vec<RitualRunInfo> = load_runs_from_db(layout, binary_filter)?
        .iter()
        .map(|r| db_run_to_info(layout, r))
        .collect();

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

fn validate_run_status(status: &str) -> Result<()> {
    const ALLOWED: &[&str] = &["pending", "running", "succeeded", "failed", "canceled", "stubbed"];
    if ALLOWED.contains(&status) {
        Ok(())
    } else {
        Err(anyhow!("Invalid status '{}'. Allowed: {:?}", status, ALLOWED))
    }
}

/// Helper to print whether a directory exists.
fn print_dir_status(label: &str, path: &Path) {
    let exists = path.is_dir();
    println!("- {label}: {} ({})", if exists { "OK" } else { "MISSING" }, path.display());
}
#[derive(Debug, Deserialize, Serialize)]
struct RitualSpec {
    name: String,
    binary: String,
    roots: Vec<String>,
    #[serde(default)]
    max_depth: Option<u32>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    outputs: Option<RitualOutputs>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RitualOutputs {
    #[serde(default)]
    reports: bool,
    #[serde(default)]
    graphs: bool,
    #[serde(default)]
    docs: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct RitualRunMetadata {
    ritual: String,
    binary: String,
    spec_hash: String,
    binary_hash: Option<String>,
    started_at: String,
    finished_at: String,
    status: String,
}

impl RitualSpec {
    fn validate(&self) -> Result<()> {
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

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
