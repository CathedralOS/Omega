#!/bin/bash
# Bisect witness with fixed PIN ledger + PIN broken observation.
set -euo pipefail
WITNESS="${1:?witness fixture}"
OUT_JSON="${2:?output json}"
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
TOOLCACHE="/tmp/ledger-bisect-tools"
mkdir -p "$TOOLCACHE"
rsync -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools/" "$TOOLCACHE/"
chmod +x "$TOOLCACHE"/*.sh 2>/dev/null || true
cd "$ROOT"
SCRIPT="$TOOLCACHE/bisect_witness_full.sh"
SETUP="$TOOLCACHE/pin_bisect_context.py"
PIN="${BISECT_PIN:-51e8c45c793007f211d0d9baee7de16767172444}"
ANCHOR="${BISECT_ANCHOR:-741270df60f7}"
export BISECT_WITNESS="$WITNESS"
export BISECT_TOOLCACHE="$TOOLCACHE"
BRANCH=work/ledger-bisect-part-26

single_test() {
  local expect=$1
  set +e
  "$SCRIPT" >/dev/null 2>&1
  local rc=$?
  set -e
  [[ "$rc" == "$expect" ]]
}

trial_expect() {
  local expect=$1
  local r1 r2 r3
  set +e
  "$SCRIPT" >/dev/null 2>&1; r1=$?
  "$SCRIPT" >/dev/null 2>&1; r2=$?
  "$SCRIPT" >/dev/null 2>&1; r3=$?
  set -e
  local flaky=0
  for r in "$r1" "$r2" "$r3"; do
    [[ "$r" == "$expect" ]] || flaky=1
  done
  python3 - "$r1" "$r2" "$r3" "$expect" "$flaky" <<'PY'
import json, sys
r1, r2, r3, expect, flaky = sys.argv[1:6]
print(json.dumps({"results": [int(r1), int(r2), int(r3)], "expect": int(expect), "flaky": bool(int(flaky))}))
PY
}

is_ledger_only_commit() {
  git rev-parse -q --verify HEAD^1 >/dev/null 2>&1 || return 1
  mapfile -t files < <(git diff --name-only HEAD^1 HEAD)
  ((${#files[@]} == 0)) && return 1
  for f in "${files[@]}"; do
    if [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ ! "$f" =~ \.md$ ]]; then
      return 1
    fi
  done
  return 0
}

find_good_commit() {
  chmod -R u+w "$ROOT/target" 2>/dev/null || true
  find "$ROOT/target" -type f ! -writable -exec chmod u+w {} + 2>/dev/null || true
  export MBX_SCHEDULER_CPUS=8 CARGO_BUILD_JOBS=8 OMEGA_CORPUS_JOBS=1
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null
  export BISECT_WITNESS="$WITNESS"
  "$TOOLCACHE/find_good_commit.sh" "$PIN" "$SCRIPT"
}

git checkout -qf "$BRANCH"
git checkout -qf "$PIN"
export BISECT_PIN="$PIN"
MEASURE="$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_measure.txt"
if [[ -f "$MEASURE" ]] && grep -q "^${WITNESS} " "$MEASURE"; then
  python3 "$SETUP" "$WITNESS" "$MEASURE" > "${OUT_JSON%.json}.pin_context.json"
else
  export MBX_SCHEDULER_CPUS=8 CARGO_BUILD_JOBS=8 OMEGA_CORPUS_JOBS=1
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null
  python3 "$SETUP" "$WITNESS" > "${OUT_JSON%.json}.pin_context.json"
fi
cp /tmp/ledger-bisect-context.json "${OUT_JSON%.json}.pin_context.json"

GOOD="$(find_good_commit)" || {
  echo "could not find good commit for $WITNESS; trying walk with cached PIN ledger only" >&2
  exit 2
}
echo "good=$GOOD"
if [[ "$(git rev-parse "$GOOD")" == "$(git rev-parse "$PIN")" ]]; then
  echo "good equals PIN — abort" >&2
  exit 2
fi

git checkout -qf "$GOOD"
GOOD_EP=$(trial_expect 0)
git checkout -qf "$PIN"
PIN_EP=$(trial_expect 1)
git checkout -qf "$ANCHOR"
ANCHOR_EP=$(trial_expect 1)
git checkout -qf "$BRANCH"

export GIT_PAGER=cat
git bisect reset 2>/dev/null || true
set +e
git bisect start --first-parent "$PIN" "$GOOD"
git bisect run "$SCRIPT"
BEXIT=$?
set -e
BREAK=""
if [[ $BEXIT -eq 0 ]]; then
  BREAK=$(git rev-parse HEAD)
  if is_ledger_only_commit; then
    echo "ERROR: bisect landed on ledger-only $BREAK" >&2
    BREAK=""
    BEXIT=1
  fi
elif [[ -f "${OUT_JSON}.log" ]]; then
  mapfile -t candidates < <(grep 'possible first bad commit:' "${OUT_JSON}.log" | sed -n 's/.*\[\([^]]*\)\].*/\1/p')
  for cand in "${candidates[@]}"; do
    git checkout -qf "$cand"
    if is_ledger_only_commit; then
      continue
    fi
    BREAK="$cand"
    break
  done
  if [[ -n "$BREAK" ]]; then
    BEXIT=0
  fi
fi
git bisect log > "${OUT_JSON}.log" 2>/dev/null || true
git bisect reset 2>/dev/null || true
git checkout -qf "$BRANCH"

PR=""
MECH=""
STATUS="propagated"
if [[ -n "$BREAK" && "$BEXIT" -eq 0 ]]; then
  PR=$(git log -1 --format='%s' "$BREAK" | sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' | head -1)
  MECH=$(git log -1 --format='%s' "$BREAK")
  DIFF=$(git show --stat --format= "$BREAK" 2>/dev/null | head -12 | tr '\n' '; ')
  MECH="${MECH} — ${DIFF}"
  if [[ "$(git rev-parse "$BREAK")" == "$(git rev-parse "$PIN")" ]]; then
    STATUS="propagated"
    MECH="invalid: first_bad_equals_pin"
  else
    STATUS="bisected"
  fi
fi

python3 - "$OUT_JSON" <<PY
import json
from pathlib import Path
out = Path("$OUT_JSON")
payload = {
    "witness": "$WITNESS",
    "good": "$GOOD",
    "bad": "$PIN",
    "anchor": "$ANCHOR",
    "breaking_commit": "$BREAK",
    "pr": "$PR" or None,
    "mechanism": """$MECH"""[:400],
    "status": "$STATUS",
    "bisect_exit": int($BEXIT),
    "endpoints": {
        "good": json.loads("""$GOOD_EP"""),
        "pin": json.loads("""$PIN_EP"""),
        "anchor": json.loads("""$ANCHOR_EP"""),
    },
    "pin_context_path": "${OUT_JSON%.json}.pin_context.json",
}
out.write_text(json.dumps(payload, indent=2) + "\n")
print(payload["status"], "$WITNESS", ( "$BREAK"[:12] if "$BREAK" else "?" ), "pr", "$PR")
PY
