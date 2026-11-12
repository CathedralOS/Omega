#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT="${2:?out json}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-16
export LEDGER_BISECT_ROOT="$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
bash "$HERE/tools/bootstrap_tools.sh" >/dev/null
/tmp/ledger-bisect-tools/run_bisect_pin.sh "$WITNESS" "$OUT"
