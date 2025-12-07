# ADR 0001: Initial Architecture

## Status

Accepted

## Context

We want a slice-oriented reverse-engineering assistant for native game/engine binaries, designed to:

- Work over large binaries (e.g., `libCQ2Client.so`).
- Operate deterministically with explicit evidence and no ML.
- Be driven by a scriptable DSL ("rituals") and run headless.
- Maintain a persistent project knowledge base across builds.
- Produce structured outputs (Markdown, JSON, Graphviz).

We also want:

- Cross-platform support (Windows, macOS, Linux).
- Strong testability for all non-trivial components.
- Clear separation between analysis core and user-facing CLI.

## Decision

We will implement Ritual Slicer as:

1. A Rust **workspace** with:
   - `ritual-core` — library crate containing:
     - Internal IR for binaries, functions, slices, and evidence.
     - Analysis and slice-carving logic.
     - Ritual DSL parsing and execution.
     - Integration with disassembly backends (Capstone, rizin, etc.).
     - Project database layer (SQLite).
   - `ritual-cli` — binary crate providing:
     - Command-line interface for running rituals and managing projects.
     - Export of Markdown/JSON/Graphviz artifacts.

2. A documentation structure with:
   - `docs/adr/` for architecture decision records.
   - Generated `docs/slices/<slice>.md` for slice-level outputs.

3. A CI pipeline that:
   - Builds all workspace crates.
   - Runs `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test`.

## Consequences

- The core analysis and data model can be reused by other frontends (e.g., Python bindings, GUI tools).
- The CLI stays thin and simple, focusing on IO and UX rather than analysis logic.
- Integrations with external tools (IDA, Ghidra, rizin) will be funneled through `backends` in `ritual-core`.
- Adding new slices, rituals, or backends should not require modifying the CLI beyond wiring new subcommands.
