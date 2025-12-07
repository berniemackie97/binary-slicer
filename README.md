# Binary Slicer (Slice-Oriented Reverse-Engineering Assistant)

Binary Slicer is a Rust toolkit for **slice-oriented reverse engineering** of native game/engine binaries (starting with `libCQ2Client.so`). It helps you carve a large binary into **subsystems ("slices")** with explicit evidence and repeatable workflows.

> No vibes, only receipts. Slice-first, evidence-first.

## What it does today

- Workspace layout: `ritual-core` library + `binary-slicer` binary.
- Persistent project database (`.ritual/project.db`) and config (`.ritual/project.json`).
- CLI scaffolding for projects, binaries, and slices:
  - `init-project` creates `.ritual`, docs/reports/graphs dirs, config, and DB.
  - `add-binary` registers binaries with arch + SHA-256 (or user-provided) hash.
  - `init-slice` inserts slice records and scaffolds docs under `docs/slices/<Name>.md`.
  - `list-slices` / `list-binaries` show stored metadata (human or JSON).
  - `project-info` reports core paths and directory health.
- Tests + coverage (`cargo llvm-cov --workspace --summary-only`).

Planned next milestones:
- Ritual DSL to declare roots/boundaries and traversal rules.
- Capstone/rizin backends feeding a common IR (functions, xrefs, CFG).
- Slice doc/report/graph generation from the DB + analysis runs.
- Import/export hooks for IDA/Ghidra/rizin annotations.

## Quickstart (CLI)

Binary name: `binary-slicer` (CLI branding string also `binary-slicer`).

```bash
# 1) Init a project (creates .ritual/, docs/, reports/, graphs/)
binary-slicer init-project --root /path/to/workdir --name CQ2Reverse

# 2) Register a binary (auto-hashes unless you provide/skip)
binary-slicer add-binary --root /path/to/workdir --path /path/to/libCQ2Client.so --arch armv7
# or: --hash <precomputed>    to store a provided hash
# or: --skip-hash             to avoid hashing large files

# 3) Create a slice scaffold
binary-slicer init-slice --root /path/to/workdir --name AutoUpdateManager --description "Handles OTA updates"

# 4) List what you have
binary-slicer list-slices --root /path/to/workdir
binary-slicer list-binaries --root /path/to/workdir

# JSON output for scripting
binary-slicer list-slices --root /path/to/workdir --json
binary-slicer list-binaries --root /path/to/workdir --json

# 5) Inspect project health/paths
binary-slicer project-info --root /path/to/workdir
```

Slice docs live at `docs/slices/<Name>.md` and are meant to be edited by humans while also regenerated from analysis later. Reports/graphs will be emitted to `reports/` and `graphs/` respectively once the analysis pipeline is wired.

## Project layout

```
<root>/
  .ritual/
    project.json   # project config (name, db path)
    project.db     # persistent SQLite DB (binaries, slices, future evidence)
  docs/
    slices/        # per-slice Markdown scaffolds
  reports/         # structured JSON output per slice/project (planned)
  graphs/          # DOT/Graphviz artifacts (planned)
```

## Development

Prereqs: Rust stable, `cargo`.

Build/test:
```bash
cargo fmt --all
cargo test
# lint (clippy, deny warnings)
cargo lint
# coverage (requires cargo-llvm-cov): 
cargo llvm-cov --workspace --summary-only

# one-shot local CI (fmt + clippy + test + coverage)
# scripts:
./scripts/ci-local.sh
# or:
pwsh ./scripts/ci-local.ps1
# or cargo aliases:
cargo ci-local            # bash
cargo ci-local-pwsh       # PowerShell
```

### Crate overview
- `crates/core`: core IR, analysis scaffolding, project DB (SQLite via rusqlite).
- `crates/cli`: CLI wiring to the core (init/list/add commands, hashing, JSON output).

## Design principles

- Deterministic, evidence-backed analysis (no AI/ML).
- Slice-first worldview with explicit roots, boundaries, and helpers.
- Scriptable and headless: rituals/DSL drive the pipeline; outputs are Markdown/JSON/DOT.
- Long-term memory: project DB tracks binaries, slices, functions, and evidence across builds.
- Interop over replacement: designed to live alongside IDA/Ghidra/rizin, not replace them.

## Roadmap highlights

- Ritual DSL for traversal/classification/outputs.
- Backends: Capstone + rizin first; optional Ghidra/IDA headless integration later.
- Evidence index tying doc claims to addresses/xrefs/strings.
- Cross-build diffing for binaries/slices/functions.
- Optional Python bindings for ad-hoc analysis.

