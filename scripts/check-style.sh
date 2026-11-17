#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

sources=$(git ls-files '*.rs')
status=0

comments=$(grep -nE '^[[:space:]]*//' $sources | grep -vE ':[[:space:]]*///( |$)' | grep -vE ':[[:space:]]*// (Arrange|Act|Assert)$' || true)
if [ -n "$comments" ]; then
  echo "Comments other than clap's /// help and the Arrange-Act-Assert markers (AGENTS.md):"
  echo "$comments"
  status=1
fi

elses=$(grep -nE '\}[[:space:]]*else([^[:alnum:]_]|$)' $sources || true)
if [ -n "$elses" ]; then
  echo "if/else found; Object Calisthenics allows no else (let-else is fine):"
  echo "$elses"
  status=1
fi

markers=$(awk '
  function finish() {
    if (name != "" && order != "AAA") print file ":" line ": " name " needs exactly one // Arrange, // Act, // Assert in that order"
    name = ""
  }
  FNR == 1 { finish() }
  /#\[test\]/ { finish(); pending = 1; next }
  pending && /fn [a-z_0-9]+/ { match($0, /fn [a-z_0-9]+/); name = substr($0, RSTART + 3, RLENGTH - 3); line = FNR; file = FILENAME; order = ""; pending = 0; next }
  name != "" && /^[[:space:]]*\/\/ Arrange$/ { order = order (order == "" ? "A" : "x") }
  name != "" && /^[[:space:]]*\/\/ Act$/ { order = order (order == "A" ? "A" : "x") }
  name != "" && /^[[:space:]]*\/\/ Assert$/ { order = order (order == "AA" ? "A" : "x") }
  END { finish() }
' $sources)
if [ -n "$markers" ]; then
  echo "Tests break Arrange-Act-Assert:"
  echo "$markers"
  status=1
fi

exit $status
