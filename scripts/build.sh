#!/usr/bin/env bash
set -euo pipefail

# Run from the repository root regardless of the current working directory.
cd "$(dirname "$0")/.."

echo "==> Building (debug)..."
cargo build --workspace

echo "==> Building (release)..."
cargo build --workspace --release

echo "==> Generating documentation..."
cargo doc --workspace --no-deps

echo "Build complete."
