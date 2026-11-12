#!/bin/bash
# Snapshot PIN ledger and one witness baseline JSON under /tmp.
set -euo pipefail
PIN="${1:?pin}"
WITNESS="${2:?witness}"
STATUS="${3:?pin bad status}"
DIAG="${4:?pin first diagnostic}"
TOOLS_DIR="$(dirname "$0")"
ROOT="$(git -C "$TOOLS_DIR/../../.." rev-parse --show-toplevel)"
PIN_LEDGER="/tmp/ledger_bisect_pin_corpus_outcomes.txt"
cp "$ROOT/tests/omega/corpus_outcomes.txt" "$PIN_LEDGER"
slug="$(echo "$WITNESS" | tr '/ ' '__')"
BASELINE="/tmp/ledger_bisect_witness_${slug}.json"
python3 - "$PIN_LEDGER" "$WITNESS" "$STATUS" "$DIAG" "$BASELINE" <<'PY'
import json, re, sys
from pathlib import Path

def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]

ledger = Path(sys.argv[1]).read_text()
witness, status, diag, out = sys.argv[2:6]
line = next((l for l in ledger.splitlines() if l.startswith(witness + " ")), None)
expected = None
if line:
    for f in line.split()[3:]:
        if f.startswith("regressed-from:"):
            expected = f[len("regressed-from:"):].split("+")[0]
            break
    if expected is None:
        expected = line.split()[1]
payload = {
    "witness": witness,
    "expected_status": expected,
    "pin_bad_status": status,
    "pin_bad_diag": diag,
    "pin_bad_diag_norm": norm_diag(diag),
}
Path(out).write_text(json.dumps(payload, indent=2))
print(out)
PY
