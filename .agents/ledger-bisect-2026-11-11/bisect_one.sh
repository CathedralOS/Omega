#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
OUT_JSON="${2:?out.json}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
ROOT="${BISECT_REPO_ROOT:-$MAIN_REPO}"
SCRIPT="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
FIND_GOOD="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/find_good_commit.sh"
cd "$ROOT"
START_SHA="$(git rev-parse HEAD)"
START_BRANCH="$(git rev-parse --abbrev-ref HEAD)"

export BISECT_REPO_ROOT="$ROOT"
if ! GOOD="$(nice -n 19 "$FIND_GOOD" "$WITNESS")"; then
  python3 - "$OUT_JSON" "$WITNESS" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "breaking_commit": "", "pr": None,
    "mechanism": "no first-parent good commit found (witness still regressed at ledger stamp and 200 parents of 759c80404428)",
    "status": "propagated", "bisect_exit": 2, "stability": "unknown",
}, indent=2))
print("propagated no-good", sys.argv[2])
PY
  git checkout -f "$START_SHA" >/dev/null 2>&1
  exit 0
fi

echo "witness=$WITNESS good=$GOOD bad=$PIN" >&2
export BISECT_WITNESS="$WITNESS"
export GIT_PAGER=cat
git checkout -f "$START_SHA" >/dev/null 2>&1
git bisect reset >/dev/null 2>&1 || true
BSTART=$(git bisect start --first-parent "$PIN" "$GOOD" 2>&1)
echo "$BSTART" >&2
if ! echo "$BSTART" | grep -qiE 'Bisecting|revisions left'; then
  python3 - "$OUT_JSON" "$WITNESS" "$GOOD" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "good_commit": sys.argv[3], "breaking_commit": "", "pr": None,
    "mechanism": f"git bisect start failed between PIN and good {sys.argv[3][:12]}",
    "status": "propagated", "bisect_exit": 2,
}, indent=2))
PY
  exit 0
fi

# Sanity: good endpoint should test clean (0), bad endpoint bad (1)
export BISECT_WITNESS="$WITNESS"
git checkout -q "$GOOD"
g1=$("$SCRIPT"; echo $?)
git checkout -q "$PIN"
b1=$("$SCRIPT"; echo $?)
git checkout -f "$START_SHA" >/dev/null 2>&1
echo "sanity good=$g1 bad=$b1" >&2
STABILITY="deterministic"
if [[ "$g1" != "0" || "$b1" != "1" ]]; then
  git checkout -q "$GOOD"; g2=$("$SCRIPT"; echo $?); g3=$("$SCRIPT"; echo $?)
  git checkout -q "$PIN"; b2=$("$SCRIPT"; echo $?); b3=$("$SCRIPT"; echo $?)
  git checkout -f "$START_SHA" >/dev/null 2>&1
  if [[ "$g1" != "$g2" || "$g2" != "$g3" || "$b1" != "$b2" || "$b2" != "$b3" ]]; then
    STABILITY="flaky"
  fi
  if [[ "$g1" != "0" ]]; then
    python3 - "$OUT_JSON" "$WITNESS" "$GOOD" "$STABILITY" "$g1" "$b1" <<'PY'
import json, sys
open(sys.argv[1], "w").write(json.dumps({
    "witness": sys.argv[2], "good_commit": sys.argv[3], "breaking_commit": "",
    "mechanism": f"sanity failed: good exit {sys.argv[5]} bad exit {sys.argv[6]} (expected 0/1); stability={sys.argv[4]}",
    "status": "propagated", "bisect_exit": 2, "stability": sys.argv[4],
}, indent=2))
PY
    exit 0
  fi
fi

git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD" >/dev/null
set +e
git bisect run "$SCRIPT"
RC=$?
set -e
BREAK=$(git rev-parse HEAD^{commit} 2>/dev/null || true)
git bisect reset >/dev/null 2>&1 || true
git checkout -f "$START_SHA" >/dev/null 2>&1
PR=""
MECH=""
STATUS=propagated
if [[ -n "$BREAK" && "$RC" -le 1 && "$BREAK" != "$PIN" ]]; then
  STATUS=bisected
  PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  SUBJ=$(git log -1 --format='%s' "$BREAK" 2>/dev/null)
  STAT=$(git show --stat --format= "$BREAK" 2>/dev/null | tr '\n' '; ' | head -c 220)
  MECH="$SUBJ"
  [[ -n "$STAT" ]] && MECH="$MECH ($STAT)"
elif [[ -n "$BREAK" && "$BREAK" == "$PIN" ]]; then
  MECH="bisect reported PIN as first bad but witness was already broken at 741270df — check good commit or bisect_test"
fi
python3 - "$OUT_JSON" "$WITNESS" "$BREAK" "$PR" "$MECH" "$STATUS" "$RC" "$GOOD" "$STABILITY" <<'PY'
import json, sys
out, witness, brk, pr, mech, status, rc, good, stab = sys.argv[1:10]
open(out, "w").write(json.dumps({
    "witness": witness, "good_commit": good, "breaking_commit": brk, "pr": pr or None,
    "mechanism": mech[:350], "status": status, "bisect_exit": int(rc), "stability": stab,
}, indent=2))
print(status, (brk or "?")[:12], "pr", pr, "good", good[:12])
PY
