#!/bin/bash
# Bisect one witness: bad=PIN, good from ledger stamp (or $BISECT_GOOD).
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
if [[ -z "${BISECT_GOOD:-}" ]]; then
  GOOD=$(python3 - "$WITNESS" <<'PY'
import sys
fix = sys.argv[1]
for line in open("tests/omega/corpus_outcomes.txt"):
    if line.startswith(fix + " "):
        for tok in line.split():
            if tok.startswith("@"):
                print(tok[1:])
                raise SystemExit
        break
PY
)
else
  GOOD="$BISECT_GOOD"
fi
[[ -n "$GOOD" ]] || { echo "no good commit for $WITNESS" >&2; exit 2; }
cd "$ROOT"
export GIT_PAGER=cat
git bisect reset 2>/dev/null || true
if ! git bisect start "$PIN" "$GOOD" --first-parent 2>/dev/null; then
  git bisect start
  git bisect bad "$PIN"
  git bisect good "$GOOD"
fi
set +e
export BISECT_WITNESS="$WITNESS"
nice -n 19 git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git bisect log 2>/dev/null | sed -n 's/^# first bad commit: \[\([^]]*\)\].*/\1/p' | head -1)
if [[ -z "$BREAK" ]]; then
  BREAK=$(git rev-parse HEAD 2>/dev/null || true)
fi
git bisect reset 2>/dev/null || true
# resolve full hash
if [[ -n "$BREAK" ]] && [[ ${#BREAK} -lt 40 ]]; then
  BREAK=$(git rev-parse "$BREAK^{commit}" 2>/dev/null || echo "$BREAK")
fi
PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$RC" "$GOOD" <<'PY'
import json, subprocess, sys
out, witness, brk, pr, rc, good = sys.argv[1:7]
mech = ""
if brk:
    diff = subprocess.run(
        ["git", "show", "--stat", "--format=", brk],
        capture_output=True, text=True, cwd=".",
    ).stdout.strip().replace("\n", "; ")[:400]
    subj = subprocess.run(
        ["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True
    ).stdout.strip()
    mech = subj
    if diff:
        mech += f" — {diff}"
status = "bisected" if rc in (0, 1) and brk else "propagated"
payload = {
    "witness": witness,
    "good": good,
    "breaking_commit": brk,
    "pr": pr or None,
    "mechanism": mech[:500],
    "status": status,
    "bisect_exit": int(rc),
}
open(out, "w").write(json.dumps(payload, indent=2))
print(json.dumps(payload, indent=2))
PY
