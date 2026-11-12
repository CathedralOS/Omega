#!/bin/bash
# Pin fixed ledger + witness PIN observation under /tmp before bisect.
set -euo pipefail
WITNESS="${1:?witness path}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git show "$PIN:tests/omega/corpus_outcomes.txt" > /tmp/ledger_bisect_corpus_outcomes.txt
REC="$ROOT/.agents/ledger-bisect-2026-11-11/pin_measure.txt"
python3 - "$WITNESS" "$REC" <<'PY'
import json, sys
from pathlib import Path

witness, rec_path = sys.argv[1], Path(sys.argv[2])
text = rec_path.read_text().splitlines()
header = next((l for l in text if l.startswith(witness + " ")), None)
if not header:
    raise SystemExit(f"missing {witness} in pin_measure")
parts = header.split()
pin_status = parts[1]
first_diag = ""
for i, line in enumerate(text):
    if line == header:
        for j in range(i + 1, len(text)):
            if text[j].startswith("\t"):
                first_diag = text[j].lstrip("\t").strip()
                break
            if text[j].strip() and not text[j].startswith("\t"):
                break
        break
ledger = Path("/tmp/ledger_bisect_corpus_outcomes.txt").read_text()
ledger_line = next((l for l in ledger.splitlines() if l.startswith(witness + " ")), None)
if not ledger_line:
    raise SystemExit(f"missing {witness} in PIN ledger")
lp = ledger_line.split()
ledger_status = lp[1]
expected = ledger_status
for tok in lp[3:]:
    if tok.startswith("regressed-from:"):
        expected = tok.split(":", 1)[1]
        break
payload = {
    "witness": witness,
    "pin_status": pin_status,
    "first_diagnostic": first_diag,
    "ledger_status": ledger_status,
    "expected_status": expected,
    "pin_commit": sys.argv[3] if len(sys.argv) > 3 else "51e8c45c793007f211d0d9baee7de16767172444",
}
Path("/tmp/ledger_bisect_witness_pin.json").write_text(json.dumps(payload, indent=2))
print(json.dumps(payload, indent=2))
PY
