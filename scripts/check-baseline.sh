#!/usr/bin/env bash
# Usage: check-baseline.sh <passed-count>
set -euo pipefail
cd "$(dirname "$0")/.."

count="${1:-}"
if ! [[ "$count" =~ ^[0-9]+$ ]]; then
  echo "baseline: argument must be an integer, got '${count}'" >&2
  exit 2
fi

# WHY: core.autocrlf=true can turn baseline.env to CRLF, so strip \r after grep.
baseline="$(grep -E '^BASE_TEST_PASS=' scripts/baseline.env | tail -n 1 | cut -d= -f2 | tr -d '\r' || true)"
if ! [[ "$baseline" =~ ^[0-9]+$ ]]; then
  echo "baseline: BASE_TEST_PASS missing or not an integer in scripts/baseline.env" >&2
  exit 2
fi

if (( count < baseline )); then
  echo "baseline: ${count} passed, expected at least ${baseline}"
  exit 1
fi
echo "baseline: ${count} passed (baseline ${baseline})"
