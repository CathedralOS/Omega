#!/bin/bash
# Bisect one witness: PIN observation vs fixed PIN ledger; find good by exponential walk on first-parent.
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${BISECT_BAD:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
bash "$ROOT/.agents/ledger-bisect-2026-11-11/install_bisect_tools.sh"
SCRIPT="${BISECT_TOOLS_ROOT:-/tmp/ledger_bisect_tools}/bisect_witness.sh"
OUT_DIR="$ROOT/.agents/ledger-bisect-2026-11-11/bisect_results"
MEASURE="${BISECT_MEASURE:-$ROOT/.agents/ledger-bisect-2026-11-11/slice_2_pin_measure.txt}"
mkdir -p "$OUT_DIR"
SAFE="${WITNESS//\//_}"
export BISECT_WITNESS="$WITNESS"
cd "$ROOT"
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1 || true
START_SHA="$(git rev-parse HEAD)"

python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_snapshot.py" "$WITNESS" "$MEASURE"

run_test_at() {
  local rev="$1"
  git -c submodule.recurse=false checkout -f --detach "$rev" >/dev/null 2>&1 || return 125
  export BISECT_TOOLS_ROOT="/tmp/ledger_bisect_tools"
  unset BISECT_SKIP_LEDGER_ONLY
  local rc=0
  nice -n 19 "$SCRIPT" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    nice -n 19 "$SCRIPT" || rc=$?
  fi
  return "$rc"
}

fixture_add_commit() {
  local w="$1"
  git log --first-parent --reverse --diff-filter=A --format=%H -- "tests/omega/${w}" 2>/dev/null | head -1
}

fixture_max_probe_index() {
  local fix="$1"
  shift
  local revs=("$@")
  local i
  if [[ -z "$fix" ]]; then
    echo $((${#revs[@]} - 1))
    return 0
  fi
  for i in "${!revs[@]}"; do
    if [[ "${revs[i]}" != "$fix" ]] && ! git merge-base --is-ancestor "$fix" "${revs[i]}" 2>/dev/null; then
      echo $((i - 1))
      return 0
    fi
  done
  echo $((${#revs[@]} - 1))
}

echo "find_good from $PIN (exponential walk to fixture-add; git bisect PIN vs first good)" >&2
mapfile -t REVS < <(git rev-list --first-parent "${PIN}^1")
FIX_ADD="$(fixture_add_commit "$WITNESS")"
echo "  fixture_add=${FIX_ADD:-unknown}" >&2
MAX_IDX=$(fixture_max_probe_index "$FIX_ADD" "${REVS[@]}")
GOOD=""
set +e
if ((${#REVS[@]} > 0)); then
  probe=0
  step=1
  while ((probe <= MAX_IDX)); do
    run_test_at "${REVS[probe]}"
    rc=$?
    echo "  probe[$probe] ${REVS[probe]:0:12} -> $rc" >&2
    if [[ "$rc" -eq 0 ]]; then
      GOOD="${REVS[probe]}"
      break
    fi
    ((step *= 2))
    probe=$((probe + step))
  done
fi
set -e
git checkout --detach "$START_SHA" >/dev/null 2>&1
if [[ -z "$GOOD" ]]; then
  VERDICT="never-good since ${FIX_ADD:-fixture-unknown}"
  python3 - "$OUT_DIR/${SAFE}.json" "$WITNESS" "$FIX_ADD" "$VERDICT" <<'PY'
import json, sys
_, out, witness, fix_add, verdict = sys.argv
json.dump(
    {
        "witness": witness,
        "fixture_add": fix_add or None,
        "verdict": verdict,
        "status": "never_good_since_fixture_add",
        "method": "pin-fixed-ledger",
    },
    open(out, "w"),
    indent=2,
)
PY
  echo "done witness=$WITNESS verdict=$VERDICT"
  exit 0
fi

set +e
pin_rc=125
for _ in 1 2 3; do
  run_test_at "$PIN"
  pin_rc=$?
  [[ "$pin_rc" -ne 125 ]] && break
done
good_rc=125
for _ in 1 2 3; do
  run_test_at "$GOOD"
  good_rc=$?
  [[ "$good_rc" -ne 125 ]] && break
done
set -e
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1
[[ "$good_rc" -eq 0 && "$pin_rc" -eq 1 ]] || {
  echo "ERROR: endpoints good=$good_rc($GOOD) pin=$pin_rc($PIN)" >&2
  exit 2
}

STABLE=1
set +e
for _ in 1 2; do
  run_test_at "$GOOD"
  g=$?
  run_test_at "$PIN"
  p=$?
  [[ "$g" -eq 0 && "$p" -eq 1 ]] || STABLE=0
done
set -e
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1

git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD"
export BISECT_SKIP_LEDGER_ONLY=1
set +e
nice -n 19 git bisect run "$SCRIPT" >"$OUT_DIR/${SAFE}.log" 2>&1
RC=$?
unset BISECT_SKIP_LEDGER_ONLY
set -e
BREAK=$(git rev-parse HEAD 2>/dev/null || true)
git bisect reset 2>/dev/null || true
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1

PR=""
MECH=""
STATUS=propagated
if [[ "$RC" -le 1 && -n "$BREAK" && "$BREAK" != "$PIN" && "$BREAK" != "$GOOD" ]]; then
  if git diff --name-only "${BREAK}^1" "$BREAK" | rg -qv '^(tests/omega/corpus_outcomes\.txt|.*\.md)$'; then
    STATUS=bisected
    PR=$(git log -1 --format='%s' "$BREAK" 2>/dev/null | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
    MECH=$(python3 - "$BREAK" <<'PY'
import subprocess, sys
brk = sys.argv[1]
stat = subprocess.run(["git", "show", "--stat", "--format=", brk], capture_output=True, text=True).stdout
lines = [l.strip() for l in stat.splitlines() if l.strip() and "|" in l][:4]
subj = subprocess.run(["git", "log", "-1", "--format=%s", brk], capture_output=True, text=True).stdout.strip()
print(subj + (f" — {'; '.join(lines)}" if lines else ""))
PY
)
  else
    MECH="mechanism not established (ledger/markdown-only first-bad ${BREAK:0:12})"
    STATUS=propagated
  fi
fi
[[ -z "$MECH" ]] && MECH="mechanism not established"
[[ "$STABLE" -eq 0 ]] && MECH="${MECH}; endpoint stability: flaky across replays"

python3 - "$OUT_DIR/${SAFE}.json" "$WITNESS" "$GOOD" "$PIN" "$BREAK" "$PR" "$MECH" "$STATUS" "$STABLE" "$RC" "$FIX_ADD" <<'PY'
import json, sys
_, out, witness, good, pin, brk, pr, mech, status, stable, rc, fix_add = sys.argv
json.dump(
    {
        "witness": witness,
        "good": good,
        "bad_pin": pin,
        "fixture_add": fix_add or None,
        "breaking_commit": brk,
        "pr": pr or None,
        "mechanism": mech,
        "status": status,
        "stable": stable == "1",
        "bisect_rc": int(rc),
        "method": "pin-fixed-ledger",
    },
    open(out, "w"),
    indent=2,
)
PY
if [[ "$STATUS" == bisected ]]; then
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/record_bisect_evidence.py" "$OUT_DIR/${SAFE}.json" "$WITNESS" || true
fi
git checkout work/ledger-bisect-part-2 >/dev/null 2>&1
echo "done witness=$WITNESS good=${GOOD:0:12} break=${BREAK:0:12} pr=$PR status=$STATUS"
