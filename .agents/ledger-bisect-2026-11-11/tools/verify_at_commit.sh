#!/bin/bash
# Build at one commit with target/verify-<sha> and classify witness (or one fixture).
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?fixture path}"
TRIALS="${3:-1}"
ROOT="$(git rev-parse --show-toplevel)"
TOOLCACHE="/tmp/ledger-bisect-tools"
mkdir -p "$TOOLCACHE"
if [[ -f "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_bisect_context.py" ]]; then
  cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools/." "$TOOLCACHE/"
fi
export BISECT_TOOLCACHE="$TOOLCACHE"

protect_agents() {
  while IFS= read -r f; do
    git update-index --skip-worktree "$f" 2>/dev/null || true
  done < <(git ls-files ".agents/ledger-bisect-2026-11-11/" 2>/dev/null || true)
}
unprotect_agents() {
  while IFS= read -r f; do
    git update-index --no-skip-worktree "$f" 2>/dev/null || true
  done < <(git ls-files ".agents/ledger-bisect-2026-11-11/" 2>/dev/null || true)
}

AGENT_BACKUP="/tmp/ledger-bisect-agents-backup"
rm -rf "$AGENT_BACKUP"
if [[ -d "$ROOT/.agents/ledger-bisect-2026-11-11" ]]; then
  cp -a "$ROOT/.agents/ledger-bisect-2026-11-11" "$AGENT_BACKUP"
fi
unprotect_agents
git checkout -qf "$COMMIT"
SHA="$(git rev-parse HEAD)"
TD="$ROOT/target/verify-$SHA"
export BISECT_REPO_ROOT="$ROOT"
export BISECT_WITNESS="$WITNESS"
export BISECT_TOOLCACHE="$TOOLCACHE"
export MBX_SCHEDULER_CPUS=4 CARGO_BUILD_JOBS=4 OMEGA_CORPUS_JOBS=1
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD"

if [[ ! -f /tmp/ledger-bisect-context.json ]]; then
  MEASURE="$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_measure.txt"
  if [[ -f "$MEASURE" ]] && grep -q "^${WITNESS} " "$MEASURE"; then
    python3 "$TOOLCACHE/pin_bisect_context.py" "$WITNESS" "$MEASURE" >/dev/null
  else
    python3 "$TOOLCACHE/pin_bisect_context.py" "$WITNESS" >/dev/null
  fi
fi

nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || { echo "build_fail"; exit 125; }
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || { echo "runner_fail"; exit 125; }
RUNNER=$(ls -t "$TD"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "no_runner"; exit 125; }

results=()
status_line=""
first_diag=""
for ((t = 1; t <= TRIALS; t++)); do
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/verify_one_record.txt
  rm -f /tmp/verify_one_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  set +e
  rc=$(python3 "$TOOLCACHE/bisect_test.py" "$WITNESS" /tmp/verify_one_record.txt)
  set -e
  [[ -n "$rc" ]] || rc=125
  results+=("$rc")
  if [[ -f /tmp/verify_one_record.txt ]]; then
    status_line=$(grep -m1 "^${WITNESS} " /tmp/verify_one_record.txt || true)
    first_diag=$(awk -v w="$WITNESS" '$0 ~ "^"w" " {found=1; next} found && /^\t/ {print substr($0,2); exit}' /tmp/verify_one_record.txt)
  fi
done
git checkout -qf work/ledger-bisect-part-26
if [[ -d "$AGENT_BACKUP" ]]; then
  mkdir -p "$ROOT/.agents"
  cp -a "$AGENT_BACKUP/." "$ROOT/.agents/ledger-bisect-2026-11-11/"
fi
protect_agents
trap unprotect_agents EXIT

RESULTS_CSV=$(IFS=,; echo "${results[*]}")
export VERIFY_SHA="$SHA" VERIFY_RESULTS="$RESULTS_CSV" VERIFY_STATUS="$status_line" VERIFY_DIAG="$first_diag"
python3 - <<'PY'
import json, os
results = [int(x) for x in os.environ["VERIFY_RESULTS"].split(",") if x]
print(json.dumps({
    "commit": os.environ["VERIFY_SHA"],
    "results": results,
    "status_line": os.environ.get("VERIFY_STATUS", ""),
    "first_diag": os.environ.get("VERIFY_DIAG", ""),
}))
PY
exit 0
