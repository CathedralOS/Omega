#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
cp "$ROOT/tests/omega/corpus_outcomes.txt" /tmp/ledger-bisect-pin-corpus_outcomes.txt
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
export OMEGA_CORPUS_RECORDS="/tmp/ledger-bisect-pin-observation.txt"
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library" OMEGA_CORPUS_JOBS=1
bash "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_run_fixture.sh"
PIN="$PIN" python3 - <<'PY'
import json, os, re
from pathlib import Path
witness = os.environ["BISECT_WITNESS"]
text = Path("/tmp/ledger-bisect-pin-observation.txt").read_text()
ledger = Path("/tmp/ledger-bisect-pin-corpus_outcomes.txt").read_text()
def norm(d):
    d = re.sub(r"`[^`]*`", "`_`", d.strip())
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]
expect = None
for line in ledger.splitlines():
    if line.startswith(witness + " "):
        parts = line.split()
        expect = parts[1]
        for j, p in enumerate(parts):
            if p == "regressed-from:" and j + 1 < len(parts):
                expect = parts[j + 1]
        break
pin_status = pin_diag = None
lines = text.splitlines()
header = next((l for l in lines if l.startswith(witness + " ")), None)
if header:
    pin_status = header.split()[1]
    i = lines.index(header) + 1
    if i < len(lines) and lines[i].startswith("\t"):
        pin_diag = lines[i][1:].strip()
Path("/tmp/ledger-bisect-pin-meta.json").write_text(json.dumps({
    "witness": witness,
    "pin_commit": os.environ.get("PIN", "51e8c45c793"),
    "ledger_expect": expect,
    "pin_status": pin_status,
    "pin_first_diagnostic": pin_diag,
    "pin_diag_norm": norm(pin_diag) if pin_diag else "",
}, indent=2))
PY
