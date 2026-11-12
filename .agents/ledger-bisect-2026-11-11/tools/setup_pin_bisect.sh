#!/bin/bash
# Refresh /tmp/pin_corpus_outcomes.txt and witness snapshot from the PIN tree.
set -euo pipefail
PIN="${PIN_COMMIT:?51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${1:?witness fixture path}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git show "$PIN:tests/omega/corpus_outcomes.txt" > /tmp/pin_corpus_outcomes.txt
MEASURE="${ROOT}/.agents/ledger-bisect-2026-11-11/pin_measure.txt"
python3 "$(dirname "$0")/write_witness_snapshot.py" "$WITNESS" "$MEASURE" /tmp/pin_witness_snapshot.json
