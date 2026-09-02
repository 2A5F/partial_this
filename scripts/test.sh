#!/usr/bin/env bash
set -euo pipefail

# Run from the repository root regardless of the current working directory.
cd "$(dirname "$0")/.."

echo "==> Testing (debug)..."
cargo test --workspace

echo "==> Testing (release)..."
cargo test --workspace --release

echo "==> Running clippy..."
cargo clippy --workspace --all-targets -- -D warnings

# Miri requires the nightly toolchain plus the `miri` component; skip if absent.
if cargo +nightly miri --version >/dev/null 2>&1; then
    echo "==> Running Miri tests..."
    cargo +nightly miri test --workspace
else
    echo "Miri is not available (needs nightly + 'miri' component). Skipping." >&2
fi

echo "Tests complete."
