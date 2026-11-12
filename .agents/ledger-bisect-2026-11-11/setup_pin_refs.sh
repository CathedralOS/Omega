#!/bin/bash
set -euo pipefail
PIN="${1:?}"
shift
ROOT=/agent/repos/omega
BASE=/tmp/ledger-bisect-part-30
mkdir -p "$BASE"
git -C "$ROOT" show "${PIN}:tests/omega/corpus_outcomes.txt" >"$BASE/pin_corpus_outcomes.txt"
python3 - "$@" <<'PY'
import sys
from pathlib import Path
base = Path("/tmp/ledger-bisect-part-30")
obs = Path("/agent/repos/omega/.agents/ledger-bisect-2026-11-11/part30_observed.txt")
text = obs.read_text()
for w in sys.argv[1:]:
    lines = []
    take = False
    for line in text.splitlines():
        if line.startswith(w + " "):
            take = True
            lines.append(line)
            continue
        if take:
            if line.startswith("\t"):
                lines.append(line)
            elif line.strip():
                break
    if not lines:
        raise SystemExit(f"missing PIN observation for {w}")
    p = base / f"pin_obs_{w.replace('/', '__')}.txt"
    p.write_text("\n".join(lines) + "\n")
    print(p)
PY
