#!/bin/bash
# Bisect one witness: bad=PIN, good from ledger stamp (or $BISECT_GOOD).
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?output json}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN="51e8c45c793007f211d0d9baee7de16767172444"
TOOL_SRC="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
TMP_TOOLS="/tmp/ledger-bisect-tools-7"
rm -rf "$TMP_TOOLS"
mkdir -p "$TMP_TOOLS"
cp -a "$TOOL_SRC/bisect_witness.sh" "$TOOL_SRC/bisect_test.py" "$TMP_TOOLS/"
chmod +x "$TMP_TOOLS/bisect_witness.sh"
# bisect checkouts drop this branch's .agents tree; keep helpers outside the worktree.
SCRIPT="$TMP_TOOLS/bisect_witness.sh"
export BISECT_WITNESS="$WITNESS"
if [[ -z "${BISECT_GOOD:-}" ]]; then
  GOOD=$(python3 - "$WITNESS" <<'PY'
import subprocess
import sys

fix = sys.argv[1]
stamp = None
for line in open("tests/omega/corpus_outcomes.txt"):
    if line.startswith(fix + " "):
        for tok in line.split():
            if tok.startswith("@"):
                stamp = tok[1:]
                break
        break
FALLBACK = "759c80404428e6d9922a8b67cce32daf7e41a02e"
if stamp:
    for cand in (stamp, stamp[:12], FALLBACK):
        try:
            print(subprocess.check_output(["git", "rev-parse", f"{cand}^{{commit}}"], text=True).strip())
            raise SystemExit
        except subprocess.CalledProcessError:
            continue
print(FALLBACK)
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
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || true)
git bisect log >"${OUT_JSON%.json}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
PR=""
if [[ -n "$BREAK" ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$RC" "$GOOD" <<'PY'
import json, re, subprocess, sys

out, witness, brk, pr, rc, good = sys.argv[1:7]
mech = ""
if brk:
    subj = subprocess.run(
        ["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True
    ).stdout.strip()
    names = subprocess.run(
        ["git", "show", "--name-only", "--format=", brk],
        capture_output=True,
        text=True,
    ).stdout.strip().splitlines()
    top = [n for n in names if n and not n.startswith(".")][:4]
    path_hint = ", ".join(top) if top else ""
    mech = subj
    if path_hint and path_hint not in subj:
        mech = f"{subj} ({path_hint})"
    mech = re.sub(r"\s+", " ", mech)[:500]
status = "bisected" if rc == 0 and brk else "propagated"
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
