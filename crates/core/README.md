# ritual-core

Core library for Binary Slicer. Provides the foundational IR, project database, and analysis scaffolding that CLI and future backends use.

## Modules (current snapshot)

- `model` — placeholder IR types for binaries, slices, and functions (to be expanded with addresses, xrefs, CFG).
- `analysis` — initial analysis hooks (currently `hello_slice` stub; will host graph traversal + slice carving).
- `db` — project database and layout helpers (SQLite via `rusqlite`), managing binaries/slices and schema migrations.

## Status

Early scaffolding with a working SQLite-backed project DB and slice/binary records. Analysis and IR are minimal and will evolve alongside the ritual DSL and backend integrations (Capstone/rizin first).

## Tests

```bash
cargo test -p ritual-core
```
