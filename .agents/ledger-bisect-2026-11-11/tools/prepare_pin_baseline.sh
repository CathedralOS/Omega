#!/bin/bash
# Freeze PIN ledger and per-witness PIN observations under /tmp/ledger-bisect-pin/.
set -euo pipefail
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOL="$(cd "$(dirname "$0")" && pwd)"
BASE=/tmp/ledger-bisect-pin
WIT="$BASE/witnesses"
mkdir -p "$WIT"
git -C "$ROOT" show "$PIN:tests/omega/corpus_outcomes.txt" >"$BASE/corpus_outcomes.txt"
export LEDGER_PIN_FILE="$BASE/corpus_outcomes.txt"
python3 "$TOOL/prepare_witness_json.py" /tmp/ledger-audit/part0_pin_observed.txt
