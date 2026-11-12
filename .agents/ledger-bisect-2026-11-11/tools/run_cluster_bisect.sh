#!/bin/bash
# Bisect one witness: good must pass (0), bad must fail (1). Uses --first-parent.
set -euo pipefail
WITNESS="${1:?witness}"
GOOD="${2:?good sha}"
BAD="${3:?bad sha}"
ROOT="${REPO_ROOT:-/agent/repos/omega}"
cd "$ROOT"
BRANCH="${GIT_BRANCH:-work/ledger-bisect-part-14}"
git checkout -f "$BRANCH" >/dev/null 2>&1 || true
export BISECT_WITNESS="$WITNESS"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
chmod +x "$SCRIPT"
git bisect reset >/dev/null 2>&1 || true
# Verify ends: 0=good matches ledger, 1=worse than ledger
for label in GOOD BAD; do
  c="${!label}"
  git checkout -f "$c" >/dev/null
  set +e
  bash "$SCRIPT" >/dev/null 2>&1
  rc=$?
  set -e
  if [[ "$label" == GOOD && "$rc" -ne 0 ]]; then
    echo "ERROR: $label $c rc=$rc (expected 0)" >&2
    git checkout -f "$BRANCH" >/dev/null
    exit 2
  fi
  if [[ "$label" == BAD && "$rc" -ne 1 ]]; then
  for _ in 1 2; do
      bash "$SCRIPT" >/dev/null 2>&1 || true
    done
    set +e
    bash "$SCRIPT" >/dev/null 2>&1
    rc=$?
    set -e
    if [[ "$rc" -ne 1 ]]; then
      echo "ERROR: $label $c rc=$rc (expected 1)" >&2
      git checkout -f "$BRANCH" >/dev/null
      exit 2
    fi
  fi
done
git checkout -f "$BRANCH" >/dev/null
git bisect start --first-parent "$BAD" "$GOOD"
set +e
git bisect run bash "$SCRIPT"
BISECT_RC=$?
set -e
BREAK="$(git rev-parse HEAD^{commit})"
git bisect reset >/dev/null 2>&1 || true
git checkout -f "$BRANCH" >/dev/null 2>&1
PR="$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)"
SUBJ="$(git log -1 --format='%s' "$BREAK")"
STAT="$(git show --stat --format= "$BREAK" 2>/dev/null | tr '\n' ' ' | head -c 200)"
python3 - "$WITNESS" "$BREAK" "$PR" "$SUBJ" "$STAT" "$BISECT_RC" <<'PY'
import json, sys
witness, brk, pr, subj, stat, rc = sys.argv[1:]
open(f"/tmp/bisect_result_{witness.replace('/','_')}.json","w").write(json.dumps({
  "witness": witness, "breaking_commit": brk, "pr": pr, "subject": subj,
  "stat": stat, "bisect_exit": int(rc),
}, indent=2))
print(brk[:12], f"PR#{pr}" if pr else "no-pr", subj[:80])
PY
