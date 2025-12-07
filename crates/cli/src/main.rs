use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use binary_slicer::{canonicalize_or_current, infer_project_name, sha256_file};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde::Serialize;

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

    /// Register a binary (e.g., libCQ2Client.so) in the project database.
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
        Command::RunRitual { root, file } => run_ritual_command(&root, &file)?,
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
fn run_ritual_command(root: &str, file: &str) -> Result<()> {
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
    let spec_file = fs::File::open(spec_path)
        .with_context(|| format!("Failed to open ritual spec at {}", spec_path.display()))?;
    let spec: RitualSpec = if spec_path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_reader(spec_file).context("Failed to parse ritual spec JSON")?
    } else {
        serde_yaml::from_reader(spec_file).context("Failed to parse ritual spec YAML")?
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

    println!("Ran ritual (stub): {}", spec_copy.name);
    println!("  Binary: {}", target_bin.name);
    println!("  Roots: {:?}", spec_copy.roots);
    println!("  Output: {}", run_output_root.display());

    Ok(())
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
