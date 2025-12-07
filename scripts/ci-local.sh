#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
# coverage gates (workspace and core)
cargo llvm-cov --package ritual-core --fail-under-lines 85 --fail-under-functions 80 --fail-under-branches 75
cargo llvm-cov --workspace --summary-only --fail-under-lines 80 --fail-under-functions 75 --fail-under-branches 70