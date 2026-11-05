#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

test_log="$(mktemp)"
trap 'rm -f "$test_log"' EXIT

cargo fmt --check
cargo clippy --all-targets -- -D warnings
# WHY: pipefail keeps a failing cargo test fatal even though tee succeeds.
cargo test 2>&1 | tee "$test_log"
passed="$(grep -E '^test result: ok\. [0-9]+ passed' "$test_log" | sed -E 's/^test result: ok\. ([0-9]+) passed.*/\1/' | awk '{s+=$1} END {print s+0}')"
bash scripts/check-baseline.sh "$passed"
cargo build --release
target/release/docx-lint --help > /dev/null
echo "smoke: release binary OK"
echo "VALIDATE OK"
