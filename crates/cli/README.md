# binary-slicer

Command-line frontend for Binary Slicer. This crate wires the workspace core library (`ritual-core`) into a small set of pragmatic commands for project scaffolding and metadata management.

## Commands

- `init-project` — create `.ritual/` config/DB plus docs/reports/graphs directories.
- `project-info` — show core paths and directory health.
- `add-binary` — register a binary with optional `--arch`, `--hash`, or `--skip-hash` (default: SHA-256).
- `init-slice` — create a slice record (Planned) and scaffold `docs/slices/<Name>.md`.
- `list-slices` - list slice records (`--json` for machine-readable output).
- `list-binaries` - list registered binaries (`--json` for machine-readable output).
- `hello` - smoke test (default command if none provided).
- `list-ritual-specs`, `list-ritual-runs`, `show-ritual-run`, `update-ritual-run-status`, `clean-outputs` for managing rituals/runs/outputs.

Binary name is `binary-slicer`. Run `binary-slicer --help` for full usage.

## Examples

```bash
binary-slicer init-project --root /work/binary --name ExampleReverse
binary-slicer add-binary --root /work/binary --path /binaries/libExampleGame.so --arch armv7
binary-slicer init-slice --root /work/binary --name AutoUpdateManager --description "OTA logic"
binary-slicer list-slices --root /work/binary
binary-slicer list-binaries --root /work/binary --json
```

## Tests

```bash
cargo test -p binary-slicer
```
