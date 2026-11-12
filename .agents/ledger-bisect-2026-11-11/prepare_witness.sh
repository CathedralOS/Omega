#!/bin/bash
# Run on PIN with BISECT_WITNESS set. Writes fixed /tmp bisect inputs.
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
[[ "$(git rev-parse HEAD)" == "$PIN" ]] || git checkout -q "$PIN"
cp tests/omega/corpus_outcomes.txt /tmp/bisect_pin_corpus_outcomes.txt
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/prepare_witness.py" "$WITNESS" /tmp/part20_pin.txt /tmp/bisect_pin_witness.json
echo "prepared $WITNESS -> /tmp/bisect_pin_witness.json"
