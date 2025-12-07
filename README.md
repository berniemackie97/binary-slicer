# Ritual Slicer (Slice-Oriented Reverse-Engineering Assistant)

Ritual Slicer is a Rust-based toolkit for **slice-oriented reverse engineering** of native game/engine binaries
(starting with `libCQ2Client.so`). Its primary job is to help you carve a large binary into **subsystems ("slices")**
backed by explicit evidence:

- Roots and boundaries are defined via a small **ritual DSL**.
- Call graphs and data/xref relationships are analyzed deterministically.
- Inferences are always accompanied by **receipts** (addresses, xrefs, patterns).
- Output is designed for long-term, scriptable workflows: Markdown, JSON, Graphviz, and a persistent project database.

The philosophy:

> No vibes, only receipts. Slices first, everything else second.

## Status

Early skeleton / bootstrap:

- Rust workspace set up (`core` library crate + `cli` binary crate).
- CI workflow for `fmt`, `clippy`, and `test`.
- Initial module layout for:
  - `model` (IR for binaries, functions, slices, evidence)
  - `analysis` (graph/slice logic)
  - `rituals` (DSL)
  - `db` (project knowledge base)
  - `backends` (Capstone/rizin/IDA/Ghidra integration)
- `cli` has a basic `ritual-slicer --version` and `ritual-slicer hello` smoke command.

## Quickstart

### Prerequisites

- Rust (stable) with `cargo` installed  
  See <https://www.rust-lang.org/tools/install>.

### Build and test

```bash
# from repo root
cargo build
cargo test

```

