#!/bin/bash
# Pin ledger + witness observation for bisect (methodology: fixed PIN copies in /tmp).
set -euo pipefail
PIN="${PIN_COMMIT:-51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${1:?witness fixture}"
OBS_RECORDS="${2:?path to corpus records at PIN}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git show "$PIN:tests/omega/corpus_outcomes.txt" > /tmp/ledger_bisect_pin_corpus_outcomes.txt
python3 - "$WITNESS" "$OBS_RECORDS" <<'PY'
import json, re, sys
from pathlib import Path

witness, rec_path = sys.argv[1], Path(sys.argv[2])
text = rec_path.read_text()
status = None
diag = ""
for i, line in enumerate(text.splitlines()):
    if line.startswith(witness + " "):
        status = line.split()[1]
        if i + 1 < len(text.splitlines()) and text.splitlines()[i + 1].startswith("\t"):
            diag = text.splitlines()[i + 1][1:].strip()
        break
if not status:
    raise SystemExit(f"missing PIN observation for {witness}")

def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]

out = {
    "witness": witness,
    "status": status,
    "diag": diag,
    "diag_norm": norm_diag(diag),
    "pin": "51e8c45c793007f211d0d9baee7de16767172444",
}
Path("/tmp/ledger_bisect_witness_pin.json").write_text(json.dumps(out, indent=2))
print(json.dumps(out, indent=2))
PY
