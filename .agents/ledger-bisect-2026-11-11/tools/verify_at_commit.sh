#!/bin/bash
# Build at one commit with target/verify-<sha> and classify witness (or one fixture).
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?fixture path}"
TRIALS="${3:-1}"
ROOT="$(git rev-parse --show-toplevel)"
TOOLCACHE="/tmp/ledger-bisect-tools"
mkdir -p "$TOOLCACHE"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools/." "$TOOLCACHE/" 2>/dev/null || \
  git -C "$ROOT" archive HEAD .agents/ledger-bisect-2026-11-11/tools | tar -x -C /tmp/.agents-tools-extract && \
  cp -a /tmp/.agents-tools-extract/.agents/ledger-bisect-2026-11-11/tools/. "$TOOLCACHE/"
export BISECT_TOOLCACHE="$TOOLCACHE"

WORK_ROOT="${LEDGER_BISECT_WORK:-$ROOT/.agents/ledger-bisect-2026-11-11/work}"

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
if [[ -d "$WORK_ROOT" ]]; then
  mkdir -p "$AGENT_BACKUP"
  cp -a "$WORK_ROOT" "$AGENT_BACKUP/"
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

emit_json_fail() {
  local reason="$1"
  python3 - "$SHA" "$TRIALS" "$reason" <<'PY'
import json, sys
sha, trials, reason = sys.argv[1], int(sys.argv[2]), sys.argv[3]
print(json.dumps({
    "commit": sha,
    "results": [125] * trials,
    "status_line": "",
    "first_diag": "",
    "error": reason,
}))
PY
  exit 0
}

MEASURE="$WORK_ROOT/pin_measure.txt"
rm -f /tmp/ledger-bisect-context.json
if [[ -f "$MEASURE" ]] && grep -q "^${WITNESS} " "$MEASURE"; then
  python3 "$TOOLCACHE/pin_bisect_context.py" "$WITNESS" "$MEASURE" >/dev/null
else
  python3 "$TOOLCACHE/pin_bisect_context.py" "$WITNESS" >/dev/null
fi

nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || emit_json_fail build_fail
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || emit_json_fail runner_fail
RUNNER=$(ls -t "$TD"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || emit_json_fail no_runner

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
  mkdir -p "$ROOT/.agents/ledger-bisect-2026-11-11/work"
  cp -a "$AGENT_BACKUP/." "$ROOT/.agents/ledger-bisect-2026-11-11/work/"
fi
if [[ ! -f "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_test.py" ]]; then
  git -C "$ROOT" archive HEAD .agents/ledger-bisect-2026-11-11/tools | tar -x -C "$ROOT"
fi
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11/tools/." "$TOOLCACHE/"
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
