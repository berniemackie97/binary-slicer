# Binary Slicer Documentation

This directory contains documentation and design notes for Binary Slicer.

## Structure

- [`adr/`](./adr/) — Architecture Decision Records (ADRs) for key design choices.
- [`slices/`](./slices/) — Generated slice documentation (`docs/slices/<slice>.md`).
- `README.md` (this file) — high-level docs index and quick links.

## Current features to document

- **Project scaffolding**: `binary-slicer init-project` creates `.ritual/`, docs/reports/graphs, rituals, outputs/binaries layout, and initializes the SQLite project DB.
- **Binary registry**: `add-binary` records binaries (path/name/arch/hash) in the DB.
- **Slice scaffolding**: `init-slice` creates DB records + `docs/slices/<Name>.md`.
- **Regeneration**: `emit-slice-docs` and `emit-slice-reports` rebuild docs/reports from DB slices.
- **Ritual runs (stub)**:
  - `run-ritual` loads a YAML/JSON ritual spec, validates it, and writes normalized `spec.yaml` + `report.json` + `run_metadata.json` under `outputs/binaries/<binary>/<ritual>/` (use `--force` to overwrite an existing run).
  - `list-ritual-specs` enumerates specs under `rituals/`.
  - `list-ritual-runs` enumerates per-binary runs (human/JSON).
  - `show-ritual-run` prints details/metadata for a single run (human/JSON).
  - `clean-outputs` removes run outputs (scoped by binary/ritual or `--all`, requires `--yes`).
- **Inspection**: `project-info`, `list-slices`, `list-binaries` (human/JSON).

## Layout (project root)

```
<root>/
  .ritual/            # project config + SQLite DB
  docs/
    slices/           # generated slice docs (editable)
  reports/            # generated JSON reports per slice (regenerated)
  graphs/             # planned Graphviz/DOT outputs
  rituals/            # user-authored ritual specs (YAML/JSON)
  outputs/
    binaries/
      <binary>/<ritual>/  # normalized spec + per-run outputs
```

## Future docs to add

- Ritual DSL reference and examples.
- Backend integration notes (rizin, Capstone, IDA, Ghidra).
- Project DB schema and migration notes.
- End-to-end workflows (e.g., `libExampleGame.so`).
- Evidence index format and cross-build diffing approach.
