#!/usr/bin/env bash
# Build the optimized development CLI, then run every applicable sample's native
# oracle through the repository-owned sample gate. Sample coverage does not
# belong to the shipped `omega` command surface.
set -euo pipefail

version=$(mbx --version 2>/dev/null) || {
  echo "error: mbx 1.7.0 or newer is required; direct Cargo fallback is forbidden" >&2
  exit 1
}
number=${version#mbx }
major=${number%%.*}
remainder=${number#*.}
minor=${remainder%%.*}
case "$major" in
  ''|*[!0-9]*)
    echo "error: could not parse mbx version: $version" >&2
    exit 1
    ;;
esac
case "$minor" in
  ''|*[!0-9]*)
    echo "error: could not parse mbx version: $version" >&2
    exit 1
    ;;
esac
if [ "$major" -lt 1 ] || { [ "$major" -eq 1 ] && [ "$minor" -lt 7 ]; }; then
  echo "error: mbx 1.7.0 or newer is required; found $version" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
# The gate builds the opt-level 1 CLI it measures into build/gate-cli/.
exec python3 tools/sample_native_gate.py
