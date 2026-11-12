#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$ROOT"
git show "$PIN:tests/omega/corpus_outcomes.txt" >/tmp/ledger-bisect-pin-corpus_outcomes.txt
OBS="$ROOT/.agents/ledger-bisect-2026-11-11/part21_observed.txt"
python3 - "$WITNESS" "$OBS" <<'PY'
import json, sys
witness, obs_path = sys.argv[1], sys.argv[2]
lines = open(obs_path).read().splitlines()
header = next(l for l in lines if l.startswith(witness + " "))
status = header.split()[1]
diags = []
past = False
for l in lines:
    if l.startswith(witness + " "):
        past = True
        continue
    if past:
        if l.startswith("\t"):
            diags.append(l[1:].strip())
        elif l.strip():
            break
spec = {
    "witness": witness,
    "pin_status": status,
    "pin_first_diag": diags[0] if diags else "",
}
path = f"/tmp/ledger-bisect-spec-{witness.replace('/', '_')}.json"
open(path, "w").write(json.dumps(spec, indent=2))
print(path)
PY
