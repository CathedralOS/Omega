#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT="${2:?out json}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-16
export LEDGER_BISECT_ROOT="$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bootstrap_tools.sh" >/dev/null
/tmp/ledger-bisect-tools/run_bisect_pin.sh "$WITNESS" "$OUT"
