# Binary Slicer (Slice-Oriented Reverse-Engineering Assistant)

[![CI](https://github.com/berniemackie97/binary-slicer/actions/workflows/ci.yml/badge.svg)](https://github.com/berniemackie97/binary-slicer/actions/workflows/ci.yml)

Binary Slicer is a Rust toolkit for **slice-oriented reverse engineering** of native game/engine binaries (e.g., `libExampleGame.so`). It helps you carve a large binary into **subsystems ("slices")** with explicit evidence and repeatable workflows.

> No vibes, only receipts. Slice-first, evidence-first.

## What it does today

- Workspace layout: `ritual-core` library + `binary-slicer` binary.
- Persistent project database (`.ritual/project.db`) and config (`.ritual/project.json`).
- CLI scaffolding for projects, binaries, slices, and ritual runs:
  - `init-project` creates `.ritual`, docs/reports/graphs dirs, config, and DB.
  - `add-binary` registers binaries with arch + SHA-256 (or user-provided) hash.
  - `init-slice` inserts slice records and scaffolds docs under `docs/slices/<Name>.md`.
  - `emit-slice-docs` / `emit-slice-reports` regenerate docs and JSON reports from the DB.
  - `list-slices` / `list-binaries` show stored metadata (human or JSON).
  - `project-info` reports core paths and directory health (human or JSON).
  - `run-ritual` loads a ritual spec (YAML/JSON), validates it, and creates a per-binary output scaffold under `outputs/binaries/<binary>/<ritual>/` (use `--force` to overwrite an existing run). Emits `spec.yaml`, `report.json`, and `run_metadata.json` (hashes + timestamps).
  - `list-ritual-specs` lists ritual specs under `rituals/` (human/JSON).
  - `list-ritual-runs` enumerates runs discovered under `outputs/binaries` (human/JSON).
  - `show-ritual-run` prints metadata/paths for a single run (human/JSON).
  - `clean-outputs` safely deletes run outputs (per binary, per ritual, or all) with `--yes`.
  - Run metadata is also persisted in the project DB (binary, ritual, hashes, status, timestamps) for easy querying.
- Tests + coverage (`cargo llvm-cov --workspace --summary-only` with gates) and local CI scripts.

Planned next milestones:
- Ritual DSL to declare roots/boundaries and traversal rules.
- Capstone/rizin backends feeding a common IR (functions, xrefs, CFG).
- Slice doc/report/graph generation from the DB + analysis runs (now stubbed, later populated).
- Import/export hooks for IDA/Ghidra/rizin annotations.

## Quickstart (CLI)

Binary name: `binary-slicer` (CLI branding string also `binary-slicer`).

```bash
# 1) Init a project (creates .ritual/, docs/, reports/, graphs/)
binary-slicer init-project --root /path/to/workdir --name GameReverse

# 2) Register a binary (auto-hashes unless you provide/skip)
binary-slicer add-binary --root /path/to/workdir --path /path/to/libExampleGame.so --arch armv7
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

# 6) Generate slice docs/reports from DB records (idempotent regeneration)
binary-slicer emit-slice-docs --root /path/to/workdir
binary-slicer emit-slice-reports --root /path/to/workdir

# 7) Run a ritual spec (analysis stub) – stores normalized spec + report under outputs/binaries/<bin>/<ritual>/
cat > /path/to/workdir/rituals/telemetry.yaml <<'YAML'
name: TelemetryRun
binary: DemoBin
roots:
  - entry_point
max_depth: 3
YAML
binary-slicer run-ritual --root /path/to/workdir --file /path/to/workdir/rituals/telemetry.yaml
# re-run with --force to overwrite an existing run output directory
# binary-slicer run-ritual --root ... --file ... --force

# 8) List ritual runs (per-binary outputs)
binary-slicer list-ritual-runs --root /path/to/workdir
binary-slicer list-ritual-runs --root /path/to/workdir --binary DemoBin --json

# 9) List ritual specs
binary-slicer list-ritual-specs --root /path/to/workdir
binary-slicer list-ritual-specs --root /path/to/workdir --json

# 10) Show a specific run (paths + metadata)
binary-slicer show-ritual-run --root /path/to/workdir --binary DemoBin --ritual TelemetryRun
binary-slicer show-ritual-run --root /path/to/workdir --binary DemoBin --ritual TelemetryRun --json

# 11) Clean outputs (requires --yes; scope by binary/ritual or --all)
binary-slicer clean-outputs --root /path/to/workdir --binary DemoBin --yes
binary-slicer clean-outputs --root /path/to/workdir --binary DemoBin --ritual TelemetryRun --yes
binary-slicer clean-outputs --root /path/to/workdir --all --yes
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
  reports/         # structured JSON output per slice/project (regenerated via emit-slice-reports)
  graphs/          # DOT/Graphviz artifacts (planned)
  rituals/         # user-authored ritual specs (YAML/JSON)
  outputs/
    binaries/
      <binary_name>/
        <ritual_name>/   # per-run artifacts (normalized spec.yaml, report.json, run_metadata.json, future graphs/docs)
  demos/ (optional)      # scratch space if you want to keep demo runs side-by-side
```

## Development

Prereqs: Rust stable, `cargo`, plus optional tools:
- `cargo-llvm-cov` for coverage
- `cargo-nextest` (optional) if you want to swap in faster test runner locally
- `cargo2junit` only needed if you want local JUnit XML like CI produces

Build/test:
```bash
cargo fmt --all
cargo test
# lint (clippy, deny warnings)
cargo lint
# coverage (requires cargo-llvm-cov): 
# - core gate: lines >=85%, functions >=80%
# - workspace gate: lines >=80%, functions >=55% (function metric is noisy due to clap-generated helpers)
cargo llvm-cov --package ritual-core --fail-under-lines 85 --fail-under-functions 80
cargo llvm-cov --workspace --summary-only --fail-under-lines 80 --fail-under-functions 55

# one-shot local CI (fmt + clippy + test + coverage)
# scripts:
./scripts/ci-local.sh
# or:
pwsh ./scripts/ci-local.ps1
# or cargo aliases:
cargo ci-local            # bash
cargo ci-local-pwsh       # PowerShell
# coverage aliases:
cargo cov-core            # core-only gate
cargo cov-workspace       # workspace gate

# emit LCOV (matches CI artifact)
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

### Crate overview
- `crates/core`: core IR, analysis scaffolding, project DB (SQLite via rusqlite).
- `crates/cli`: CLI wiring to the core (init/list/add/emit/run commands, hashing, JSON output, ritual scaffolding).

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

