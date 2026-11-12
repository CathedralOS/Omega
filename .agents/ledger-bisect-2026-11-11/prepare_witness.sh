#!/bin/bash
# Run with BISECT_WITNESS set. Writes fixed /tmp bisect inputs (no checkout).
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
PREPARE_PY="${BISECT_PREPARE_PY:-/tmp/bisect_toolkit/prepare_witness.py}"
git show "$PIN:tests/omega/corpus_outcomes.txt" >/tmp/bisect_pin_corpus_outcomes.txt
[[ -f /tmp/part20_pin.txt ]] || { echo "missing /tmp/part20_pin.txt (re-measure at PIN first)" >&2; exit 1; }
python3 "$PREPARE_PY" "$WITNESS" /tmp/part20_pin.txt /tmp/bisect_pin_witness.json
echo "prepared $WITNESS -> /tmp/bisect_pin_witness.json"
