use anyhow::Result;
use binary_slicer::commands;
use clap::{Parser, Subcommand};

/// Slice-oriented reverse-engineering assistant CLI.
///
/// The binary is intentionally thin: it parses args, dispatches to command helpers,
/// and lets `ritual-core` + `commands` own the real work for testability and reuse.
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
    InitProject {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Optional project name. If omitted, the name is derived from the root directory.
        #[arg(long)]
        name: Option<String>,
    },

    /// Show basic information about an existing Binary Slicer project.
    ProjectInfo {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Register a binary (e.g., libExampleGame.so) in the project database.
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
    ListSlices {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// List all binaries registered in the project database.
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

        /// Backend to use (overrides backend in the spec). Defaults to validate-only.
        #[arg(long)]
        backend: Option<String>,

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

    /// Rerun an existing ritual using its normalized spec, storing results under a new name.
    RerunRitual {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long, default_value = ".")]
        root: String,

        /// Binary name (required).
        #[arg(long)]
        binary: String,

        /// Existing ritual run name to copy spec from.
        #[arg(long)]
        ritual: String,

        /// New ritual run name to use for the rerun (required).
        #[arg(long)]
        as_name: String,

        /// Backend to use (overrides backend in the spec). Defaults to validate-only.
        #[arg(long)]
        backend: Option<String>,

        /// Overwrite output directory if it already exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// List available analysis backends (human or JSON).
    ListBackends {
        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cmd = cli.command.unwrap_or(Command::Hello { slice: "DefaultSlice".to_string() });

    match cmd {
        Command::Hello { slice } => hello_command(&slice)?,
        Command::InitProject { root, name } => commands::init_project_command(&root, name)?,
        Command::ProjectInfo { root, json } => commands::project_info_command(&root, json)?,
        Command::AddBinary { root, path, name, arch, hash, skip_hash } => {
            commands::add_binary_command(&root, &path, name, arch, hash, skip_hash)?
        }
        Command::InitSlice { root, name, description } => {
            commands::init_slice_command(&root, &name, description)?
        }
        Command::ListSlices { root, json } => commands::list_slices_command(&root, json)?,
        Command::ListBinaries { root, json } => commands::list_binaries_command(&root, json)?,
        Command::EmitSliceDocs { root } => commands::emit_slice_docs_command(&root)?,
        Command::EmitSliceReports { root } => commands::emit_slice_reports_command(&root)?,
        Command::RunRitual { root, file, backend, force } => {
            commands::run_ritual_command(&root, &file, backend.as_deref(), force)?
        }
        Command::CleanOutputs { root, binary, ritual, all, yes } => {
            commands::clean_outputs_command(&root, binary.as_deref(), ritual.as_deref(), all, yes)?
        }
        Command::ListRitualRuns { root, binary, json } => {
            commands::list_ritual_runs_command(&root, binary.as_deref(), json)?
        }
        Command::ShowRitualRun { root, binary, ritual, json } => {
            commands::show_ritual_run_command(&root, &binary, &ritual, json)?
        }
        Command::ListRitualSpecs { root, json } => {
            commands::list_ritual_specs_command(&root, json)?
        }
        Command::UpdateRitualRunStatus { root, binary, ritual, status, finished_at } => {
            commands::update_ritual_run_status_command(
                &root,
                &binary,
                &ritual,
                &status,
                finished_at,
            )?
        }
        Command::RerunRitual { root, binary, ritual, as_name, backend, force } => {
            commands::rerun_ritual_command(
                &root,
                &binary,
                &ritual,
                &as_name,
                backend.as_deref(),
                force,
            )?
        }
        Command::ListBackends { json } => commands::list_backends_command(json)?,
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
