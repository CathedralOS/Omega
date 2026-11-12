#!/bin/bash
# Pin ledger + witness observation at PIN for fixed-outcome bisect (part k).
set -euo pipefail
WITNESS="${1:?witness fixture path}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
OBSERVED="${3:-/tmp/part32_observed.txt}"
PIN_LEDGER="/tmp/ledger-bisect-pin-corpus_outcomes.txt"
META="/tmp/ledger-bisect-witness-${WITNESS//\//_}.json"

git -C "$ROOT" show "$PIN:tests/omega/corpus_outcomes.txt" >"$PIN_LEDGER"

python3 - "$WITNESS" "$OBSERVED" "$PIN_LEDGER" "$META" <<'PY'
import json
import re
import sys
from pathlib import Path

witness, observed_path, ledger_path, meta_path = sys.argv[1:5]
obs_text = Path(observed_path).read_text()
obs_line = next((l for l in obs_text.splitlines() if l.startswith(witness + " ")), None)
if not obs_line:
    raise SystemExit(f"missing PIN observation for {witness} in {observed_path}")
parts = obs_line.split()
bad_status = parts[1]

diag_lines = []
in_rec = False
for line in obs_text.splitlines():
    if line.startswith(witness + " "):
        in_rec = True
        continue
    if in_rec:
        if line.startswith("\t"):
            diag_lines.append(line[1:].strip())
        elif line.strip():
            break
bad_first = diag_lines[0] if diag_lines else ""

ledger_line = next(
    (l for l in Path(ledger_path).read_text().splitlines() if l.startswith(witness + " ")),
    None,
)
if not ledger_line:
    raise SystemExit(f"missing ledger row for {witness}")
lp = ledger_line.split()
expected_status = lp[1]
flags = lp[3:]
regressed_from = None
for f in flags:
    if f.startswith("regressed-from:"):
        regressed_from = f.split(":", 1)[1]
        break
if regressed_from:
    expected_status = regressed_from

def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]

meta = {
    "witness": witness,
    "pin": "51e8c45c793007f211d0d9baee7de16767172444",
    "expected_status": expected_status,
    "bad_status": bad_status,
    "bad_first_diagnostic": bad_first,
    "bad_diag_norm": norm_diag(bad_first),
    "ledger_status_at_pin": lp[1],
}
Path(meta_path).write_text(json.dumps(meta, indent=2) + "\n")
print(meta_path)
PY
