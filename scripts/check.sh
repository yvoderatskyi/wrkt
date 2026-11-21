#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

stage() {
  local label=$1
  shift
  local output
  if output=$("$@" 2>&1); then
    echo "ok      $label"
    return
  fi
  echo "$output"
  echo "FAILED  $label"
  exit 1
}

stage "format" cargo fmt --check
stage "clippy" cargo clippy --all-targets --quiet -- -D warnings
stage "style" scripts/check-style.sh
stage "tests" cargo test --quiet
