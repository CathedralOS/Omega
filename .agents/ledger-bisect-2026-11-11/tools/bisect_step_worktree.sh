#!/bin/bash
# One bisect step at HEAD using a dedicated worktree + target dir. 0 good, 1 PIN-bad, 125 skip.
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="/agent/repos/omega"
COMMIT="${BISECT_HEAD:-$(git -C "$ROOT" rev-parse HEAD)}"
LB="/tmp/ledger-bisect-2026-11-11"
if [[ ! -x "$LB/tools/bisect_test.py" ]]; then
  SRC="$ROOT/.agents/ledger-bisect-2026-11-11"
  [[ -d "$SRC/tools" ]] && cp -a "$SRC" "$LB" || {
    echo "missing bisect tools in /tmp seed" >&2
    exit 125
  }
fi
parent=$(git -C "$ROOT" rev-parse --verify HEAD^1 2>/dev/null || true)
if [[ -n "$parent" ]]; then
  files=$(git -C "$ROOT" diff --name-only "$parent" HEAD 2>/dev/null || true)
  if [[ -n "$files" ]]; then
    nontrivial=$(printf '%s\n' "$files" | grep -Ev '^(tests/omega/corpus_outcomes\.txt|.*\.md)$' || true)
    [[ -z "$nontrivial" ]] && exit 125
  fi
fi
WT="/tmp/ledger-bisect-wt-${COMMIT:0:12}"
rm -rf "$WT"
git -C "$ROOT" worktree add -f "$WT" "$COMMIT" >/dev/null
export OMEGA_REPO="$WT" PIN_OBS="$LB/pin_full.txt"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
TDIR="$ROOT/target/verify-${COMMIT:0:12}"
mkdir -p "$TDIR"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || {
  git -C "$ROOT" worktree remove -f "$WT" 2>/dev/null || rm -rf "$WT"
  exit 125
}
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1 || {
  git -C "$ROOT" worktree remove -f "$WT" 2>/dev/null || rm -rf "$WT"
  exit 125
}
python3 "$LB/tools/prepare_bisect_pin.py" "$WITNESS" >/dev/null
RUNNER=$(ls -t "$TDIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || {
  git -C "$ROOT" worktree remove -f "$WT" 2>/dev/null || rm -rf "$WT"
  exit 125
}
export OMEGA_CORPUS_ROOT="$WT" OMEGA_LIBRARY_ROOT="$WT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
rc=$(python3 "$LB/tools/bisect_test.py")
git -C "$ROOT" worktree remove -f "$WT" 2>/dev/null || rm -rf "$WT"
exit "$rc"
