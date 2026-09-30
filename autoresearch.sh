#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export LC_ALL=C

# Provision the existing flake dev shell and Cargo cache once before running.
# The measured corpus is embedded in the executable; benchmark runs are offline.
if command -v cargo >/dev/null 2>&1; then
  exec cargo run --locked --offline --quiet --release -p statix --example maintainer_coverage
else
  exec nix develop --offline --command cargo run --locked --offline --quiet --release -p statix --example maintainer_coverage
fi
