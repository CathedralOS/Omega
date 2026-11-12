#!/bin/bash
# Capture PIN ledger + witness broken observation into /tmp (run on PIN checkout).
set -euo pipefail
WITNESS="${1:?witness fixture path}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git show "$PIN:tests/omega/corpus_outcomes.txt" > /tmp/ledger_bisect_corpus_outcomes.txt
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "build corpus_runner on PIN first" >&2; exit 1; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
export OMEGA_CORPUS_RECORDS=/tmp/pin_witness_record.txt
rm -f /tmp/pin_witness_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null
python3 - "$WITNESS" "$PIN" <<'PY'
import json, re, sys
from pathlib import Path

def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]

witness, pin = sys.argv[1], sys.argv[2]
text = Path("/tmp/pin_witness_record.txt").read_text()
line = next(l for l in text.splitlines() if l.startswith(witness + " "))
status = line.split()[1]
diag = ""
for follow in text.splitlines()[text.splitlines().index(line) + 1 :]:
    if follow.startswith("\t"):
        diag = follow[1:].strip()
        break
    if follow.strip():
        break
Path("/tmp/ledger_bisect_pin_observation.json").write_text(
    json.dumps(
        {
            "witness": witness,
            "pin": pin,
            "status": status,
            "first_diagnostic": diag,
            "diag_norm": norm_diag(diag),
        },
        indent=2,
    )
    + "\n"
)
print(json.dumps({"status": status, "first_diagnostic": diag}, indent=2))
PY
