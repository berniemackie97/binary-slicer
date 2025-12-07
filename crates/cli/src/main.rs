use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

/// Slice-oriented reverse-engineering assistant CLI.
///
/// This CLI is a thin wrapper around `ritual-core` (exposed in code as `ritual_core`).
/// All substantive logic lives in the library so it can be tested thoroughly
/// and reused from other frontends.
#[derive(Parser, Debug)]
#[command(
    name = "ritual-slicer",
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

    /// Initialize a new Ritual Slicer project at the given root.
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

    /// Show basic information about an existing Ritual Slicer project.
    ///
    /// This reads `.ritual/project.json` and reports key paths and config values.
    ProjectInfo {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,
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
    },

    /// Initialize a new slice and scaffold its documentation.
    ///
    /// This creates `docs/slices/<Name>.md` under the project root.
    InitSlice {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Name of the slice (e.g., AutoUpdateManager, CUIManager).
        #[arg(long)]
        name: String,

        /// Optional human-readable description of the slice.
        #[arg(long)]
        description: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Default to the Hello command if none is provided.
    match cli.command.unwrap_or(Command::Hello { slice: "DefaultSlice".to_string() }) {
        Command::Hello { slice } => hello_command(&slice)?,
        Command::InitProject { root, name } => init_project_command(&root, name)?,
        Command::ProjectInfo { root } => project_info_command(&root)?,
        Command::AddBinary { root, path, name } => add_binary_command(&root, &path, name)?,
        Command::InitSlice { root, name, description } => {
            init_slice_command(&root, &name, description)?
        }
    }

    Ok(())
}

/// "Hello" smoke-test command.
fn hello_command(slice_name: &str) -> Result<()> {
    // NOTE: crate name `ritual-core` in Cargo.toml is `ritual_core` in Rust code.
    let version = ritual_core::version();
    let result = ritual_core::analysis::hello_slice(slice_name);

    println!("ritual-slicer v{}", version);
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

    // Build project config.
    let db_path_rel = layout.db_path_relative_string();
    let config = ritual_core::db::ProjectConfig::new(&project_name, db_path_rel);

    // Serialize and write config JSON.
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&layout.project_config_path, json).with_context(|| {
        format!("Failed to write project config: {}", layout.project_config_path.display())
    })?;

    println!("Initialized Ritual Slicer project:");
    println!("  Name: {}", project_name);
    println!("  Root: {}", layout.root.display());
    println!("  Config: {}", layout.project_config_path.display());
    println!("  DB path (relative): {}", config.db.path);
    println!("  Docs dir: {}", layout.docs_dir.display());
    println!("  Slices docs dir: {}", layout.slices_docs_dir.display());
    println!("  Reports dir: {}", layout.reports_dir.display());
    println!("  Graphs dir: {}", layout.graphs_dir.display());

    Ok(())
}

/// Show basic information about an existing project.
fn project_info_command(root: &str) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Read the project config.
    let config_json = fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;

    let config: ritual_core::db::ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;

    println!("Ritual Slicer Project Info");
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

    Ok(())
}

/// Register a binary in the project database.
fn add_binary_command(root: &str, path: &str, name: Option<String>) -> Result<()> {
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
    let rel_path = match abs_path.strip_prefix(&root_path) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => abs_path.clone(),
    };
    let rel_path_str = rel_path.to_string_lossy().to_string();

    let binary_name = name.unwrap_or_else(|| {
        input_path.file_name().and_then(|os| os.to_str()).unwrap_or(path).to_string()
    });

    let record = ritual_core::db::BinaryRecord {
        name: binary_name,
        path: rel_path_str,
        arch: None,
        hash: None,
    };

    let id = db.insert_binary(&record).context("Failed to insert binary record")?;

    println!("Added binary:");
    println!("  Id: {}", id);
    println!("  Name: {}", record.name);
    println!("  Path (relative): {}", record.path);
    println!("  DB: {}", db_path.display());

    Ok(())
}

/// Initialize a new slice and scaffold its documentation under docs/slices.
///
/// This does *not* yet register the slice in the DB; that will come in a later step.
/// For now, it validates the project root and creates a Markdown stub.
fn init_slice_command(root: &str, name: &str, description: Option<String>) -> Result<()> {
    let root_path = canonicalize_or_current(root)?;
    let layout = ritual_core::db::ProjectLayout::new(&root_path);

    // Ensure this is a Ritual project by checking for project config.
    if !layout.project_config_path.exists() {
        return Err(anyhow!(
            "No Ritual project config found at {} (expected {})",
            root_path.display(),
            layout.project_config_path.display()
        ));
    }

    // Ensure docs/slices directory exists.
    fs::create_dir_all(&layout.slices_docs_dir).with_context(|| {
        format!("Failed to create slices docs directory: {}", layout.slices_docs_dir.display())
    })?;

    // Compute slice doc path.
    let file_name = format!("{name}.md");
    let doc_path = layout.slices_docs_dir.join(&file_name);

    if doc_path.exists() {
        return Err(anyhow!("Slice doc already exists: {}", doc_path.display()));
    }

    let description_line = description.unwrap_or_else(|| "TODO: describe this slice.".to_string());

    let contents = format!(
        "# Slice: {name}\n\n\
         {description_line}\n\n\
         ## Roots\n\n\
         - TODO: list known root functions (addresses, names, string refs).\n\n\
         ## Purpose\n\n\
         - TODO: high-level purpose of this subsystem.\n\n\
         ## Key Functions\n\n\
         - TODO: generated from analysis.\n\n\
         ## External Dependencies\n\n\
         - TODO: calls into other slices or libraries.\n\n\
         ## Evidence Index\n\n\
         - TODO: map claims above to concrete addresses/xrefs.\n"
    );

    fs::write(&doc_path, contents)
        .with_context(|| format!("Failed to write slice doc at {}", doc_path.display()))?;

    println!("Initialized slice:");
    println!("  Name: {name}");
    println!("  Root: {}", root_path.display());
    println!("  Doc:  {}", doc_path.display());

    Ok(())
}

/// Canonicalize the root path if possible, falling back to the given string
/// relative to the current working directory.
fn canonicalize_or_current(root: &str) -> Result<PathBuf> {
    let path = Path::new(root);
    if path == Path::new(".") {
        Ok(env::current_dir().context("Failed to get current directory")?)
    } else {
        // Try to canonicalize; if it fails (e.g., path does not yet exist),
        // join it with the current dir to get an absolute path.
        match path.canonicalize() {
            Ok(p) => Ok(p),
            Err(_) => {
                let cwd = env::current_dir().context("Failed to get current directory")?;
                Ok(cwd.join(path))
            }
        }
    }
}

/// Infer a project name from the root path.
///
/// If the root has no final component (e.g., `/`), fallback to `unnamed-project`.
fn infer_project_name(root: &Path) -> String {
    root.file_name().and_then(|os_str| os_str.to_str()).unwrap_or("unnamed-project").to_string()
}

/// Helper to print whether a directory exists.
fn print_dir_status(label: &str, path: &Path) {
    let exists = path.is_dir();
    println!("- {label}: {} ({})", if exists { "OK" } else { "MISSING" }, path.display());
}
