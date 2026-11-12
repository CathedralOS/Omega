#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT="${2:?json out}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
PIN="${LEDGER_BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
GOOD=$(python3 - "$WITNESS" <<'PY'
import sys
fix = sys.argv[1]
for line in open("tests/omega/corpus_outcomes.txt"):
    if line.startswith(fix + " "):
        for tok in line.split():
            if tok.startswith("@"):
                sha = tok[1:]
                print(sha.split("+", 1)[0])
                raise SystemExit
print("759c80404428")
PY
)
cd "$ROOT"
export BISECT_WITNESS="$WITNESS" GIT_PAGER=cat
git bisect reset 2>/dev/null || true
if ! git bisect start --first-parent "$PIN" "$GOOD" 2>/dev/null; then
  git bisect start
  git bisect bad "$PIN"
  git bisect good "$GOOD"
fi
set +e
git bisect run "$TOOLS/bisect_witness.sh"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit})
PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
SUBJ=$(git log -1 --format='%s' "$BREAK")
STAT=$(git show --stat --format= "$BREAK" | head -5 | tr '\n' ' ')
git bisect reset 2>/dev/null || true
python3 - "$OUT" "$WITNESS" "$BREAK" "$PR" "$SUBJ" "$STAT" "$RC" <<'PY'
import json, sys
out, witness, brk, pr, subj, stat, rc = sys.argv[1:8]
mech = subj
if stat.strip():
    mech = f"{subj} — {stat.strip()[:180]}"
status = "bisected" if int(rc) in (0, 1) else "propagated"
json.dump({
    "witness": witness,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:300],
    "status": status,
    "bisect_exit": int(rc),
    "good": None,
}, open(out, "w"), indent=2)
print(status, brk[:12], pr or "-")
PY
