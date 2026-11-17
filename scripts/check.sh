#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

cargo fmt --check
cargo clippy --all-targets --quiet -- -D warnings
scripts/check-style.sh
cargo test --quiet
