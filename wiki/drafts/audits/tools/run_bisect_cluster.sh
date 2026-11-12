#!/bin/bash
# Bisect one witness on first-parent main. Writes result JSON path in $2.
set -euo pipefail
WITNESS="${1:?witness fixture}"
OUT_JSON="${2:?output json}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
SCRIPT="$ROOT/wiki/drafts/audits/tools/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
if [[ -z "${BISECT_GOOD:-}" ]]; then
  GOOD=$(python3 - "$WITNESS" <<'PY'
import sys
fix = sys.argv[1]
for line in open("tests/omega/corpus_outcomes.txt"):
    if line.startswith(fix + " "):
        for tok in line.split():
            if tok.startswith("@"):
                print(tok[1:13] if len(tok) > 13 else tok[1:])
                raise SystemExit
        break
PY
)
  GOOD="${GOOD:-ccdb99ca2214}"
else
  GOOD="$BISECT_GOOD"
fi
BAD="$(git rev-parse origin/main)"
export GIT_PAGER=cat
if ! git bisect start "$BAD" "$GOOD" --first-parent 2>/dev/null; then
  git bisect reset 2>/dev/null || true
  git bisect start
  git bisect bad "$BAD"
  git bisect good "$GOOD"
fi
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || echo "")
git bisect log > "${OUT_JSON}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$RC" <<'PY'
import json, subprocess, sys
out, witness, brk, pr, rc = sys.argv[1:6]
mech = ""
if brk:
    diff = subprocess.run(
        ["git", "show", "--stat", "--format=", brk],
        capture_output=True, text=True,
    ).stdout.strip().replace("\n", "; ")[:240]
    subj = subprocess.run(
        ["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True
    ).stdout.strip()
    mech = f"{subj}" + (f" ({diff})" if diff else "")
status = "bisected" if rc in (0, 1) else "propagated"
open(out, "w").write(json.dumps({
    "witness": witness, "breaking_commit": brk, "pr": pr, "mechanism": mech[:300],
    "status": status, "bisect_exit": int(rc),
}, indent=2))
print(out, status, brk[:12] if brk else "?")
PY
