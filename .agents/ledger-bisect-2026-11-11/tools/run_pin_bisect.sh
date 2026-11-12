#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
BRANCH="${3:-work/ledger-bisect-part-14}"
OUT="${4:-/tmp/bisect_cluster_result.json}"
ROOT="${REPO_ROOT:-/agent/repos/omega}"
TOOLS="/tmp/part14-bisect-tools/tools"
export BISECT_WITNESS="$WITNESS"
if [[ -n "${BISECT_GOOD:-}" ]]; then
  GOOD="$BISECT_GOOD"
else
  GOOD="$("$TOOLS/find_pin_good.sh" "$WITNESS" "$PIN" "$BRANCH")"
fi
git -C "$ROOT" checkout -f "$BRANCH" >/dev/null
# Sanity: PIN bad, good good
git -C "$ROOT" checkout -f "$PIN" >/dev/null
set +e
bash "$TOOLS/bisect_pin_witness.sh" >/dev/null 2>&1
pin_rc=$?
set -e
[[ "$pin_rc" -eq 1 ]] || { echo "PIN not bad rc=$pin_rc" >&2; exit 2; }
git -C "$ROOT" checkout -f "$GOOD" >/dev/null
bash "$TOOLS/bisect_pin_witness.sh" >/dev/null 2>&1 && good_rc=0 || good_rc=$?
[[ "$good_rc" -eq 0 ]] || { echo "GOOD not good rc=$good_rc at $GOOD" >&2; exit 2; }
git -C "$ROOT" checkout -f "$BRANCH" >/dev/null
git -C "$ROOT" bisect reset >/dev/null 2>&1 || true
git -C "$ROOT" bisect start --first-parent "$PIN" "$GOOD"
set +e
git -C "$ROOT" bisect run bash "$TOOLS/bisect_pin_witness.sh"
rc=$?
set -e
BREAK="$(git -C "$ROOT" rev-parse HEAD^{commit})"
# Skip if ledger-only landed
mapfile -t touched < <(git -C "$ROOT" diff --name-only "${BREAK}^1" "$BREAK" 2>/dev/null || true)
compiler=0
for p in "${touched[@]}"; do
  [[ "$p" == tests/omega/corpus_outcomes.txt ]] && continue
  [[ "$p" == *.md ]] && continue
  compiler=1
done
if [[ "$compiler" -eq 0 ]]; then
  echo "WARN: landed on ledger-only $BREAK; reset and manual skip needed" >&2
fi
git -C "$ROOT" bisect reset >/dev/null 2>&1 || true
git -C "$ROOT" checkout -f "$BRANCH" >/dev/null
PR="$(git -C "$ROOT" log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)"
SUBJ="$(git -C "$ROOT" log -1 --format='%s' "$BREAK")"
DIFF="$(git -C "$ROOT" show --no-color --stat "$BREAK" | tail -1)"
python3 - "$OUT" "$WITNESS" "$GOOD" "$PIN" "$BREAK" "$PR" "$SUBJ" "$DIFF" "$rc" <<'PY'
import json, sys
out, witness, good, pin, brk, pr, subj, diff, rc = sys.argv[1:]
json.dump({
    "witness": witness, "good": good, "bad": pin,
    "breaking_commit": brk, "pr": pr or "",
    "subject": subj, "stat": diff, "bisect_exit": int(rc),
}, open(out, "w"), indent=2)
print(brk[:12], f"PR#{pr}" if pr else "", subj[:72])
PY
