#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
# coverage summary (tweak threshold inside .cargo/config if desired)
cargo llvm-cov --workspace --summary-only