# Demo: binary-slicer end-to-end run

This demo shows how to use the CLI against a real binary (`libCQ2Client.so`).

Commands executed:

```powershell
# 1) Initialize a project in ./demo
cargo run -p binary-slicer -- init-project --root demo --name DemoProject

# 2) Register the provided binary
cargo run -p binary-slicer -- add-binary --root demo --path libCQ2Client.so --name CQ2 --arch arm64

# 3) Create a slice scaffold
cargo run -p binary-slicer -- init-slice --root demo --name DemoSlice --description "Demo slice for CQ2"

# 4) Author a ritual spec (saved to demo/rituals/demo.yaml)
@'
name: DemoRun
binary: CQ2
roots:
  - entry_point
max_depth: 2
'@ | Set-Content -Path demo/rituals/demo.yaml

# 5) Run the ritual (uses validate-only backend by default)
cargo run -p binary-slicer -- run-ritual --root demo --file demo/rituals/demo.yaml

# 6) Run the ritual with Capstone backend (feature needed)
# Build with the Capstone feature and select the backend explicitly.
cargo run -p binary-slicer --features "ritual-core/capstone-backend" -- run-ritual --root demo --file demo/rituals/demo.yaml --backend capstone --force
```

Outputs produced:

- Normalized spec: `demo/outputs/binaries/CQ2/DemoRun/spec.yaml`
- Run metadata: `demo/outputs/binaries/CQ2/DemoRun/run_metadata.json` (includes backend info and hashes)
- Report: `demo/outputs/binaries/CQ2/DemoRun/report.json`
  - With Capstone backend on this stripped ARM ELF, results are minimal (root placeholder + backend version); real evidence/CFG requires symbols or a richer backend (rizin/Ghidra). Try a smaller binary with symbols to see disassembly evidence and basic blocks in the report.
- Graph (DOT): `demo/outputs/binaries/CQ2/DemoRun/graph.dot`

Notes:
- Backend defaults to `validate-only`. You can choose others with `--backend <name>` (see `binary-slicer list-backends`).
- If you enable feature flags (`capstone-backend`, `rizin-backend`, `ghidra-backend`) when building, those backends will appear in `list-backends` and can be selected per run or via `setup-backend`.
