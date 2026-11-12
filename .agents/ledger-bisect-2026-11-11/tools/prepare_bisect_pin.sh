#!/bin/bash
# Capture PIN ledger + one witness observation for fixed bisect comparison.
set -euo pipefail
WITNESS="${1:?witness fixture}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="${REPO_ROOT:-$(git -C "${REPO_ROOT:-/agent/repos/omega}" rev-parse --show-toplevel 2>/dev/null || pwd)}"
if [[ ! -d "$ROOT/.git" ]]; then
  ROOT=/agent/repos/omega
fi
cd "$ROOT"
git cat-file -e "${PIN}^{commit}" || exit 1
cp "$ROOT/tests/omega/corpus_outcomes.txt" /tmp/ledger_bisect_corpus_outcomes.txt
OBS="${3:-${LEDGER_BISECT_PIN_OBSERVED:-$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_observed.txt}}"
if [[ -f /tmp/ledger_bisect_pin_observed.txt ]]; then
  OBS=/tmp/ledger_bisect_pin_observed.txt
fi
python3 - "$WITNESS" "$OBS" "$PIN" <<'PY'
import json, re, sys
from pathlib import Path

witness, obs_path, pin_commit = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
text = obs_path.read_text() if obs_path.is_file() else ""
status, diag = None, ""
current = None
for line in text.splitlines():
    if line.startswith("\t") and current == witness:
        if not diag:
            diag = line[1:].strip()
        continue
    parts = line.split()
    if len(parts) >= 2 and parts[0].startswith(("pass/", "fail/", "run/")):
        current = parts[0]
        if current == witness:
            status, diag = parts[1], ""
if status is None:
    raise SystemExit(f"witness {witness} missing from PIN observation {obs_path}")

def norm(d: str) -> str:
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]

Path("/tmp/ledger_bisect_witness_pin.json").write_text(
    json.dumps(
        {
            "fixture": witness,
            "pin_status": status,
            "pin_first_diag": diag,
            "pin_diag_norm": norm(diag),
            "pin_commit": pin_commit,
        },
        indent=2,
    )
    + "\n"
)
print(f"prepared {witness} pin_status={status}")
PY
