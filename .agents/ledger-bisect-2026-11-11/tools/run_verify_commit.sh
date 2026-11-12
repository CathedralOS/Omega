#!/bin/bash
# Usage: run_verify_commit.sh <commit> <witness> <member1> ...
set -euo pipefail
COMMIT="${1:?}"
WITNESS="${2:?}"
shift 2
MEMBERS=("$@")
ROOT="/agent/repos/omega"
LB="/tmp/ledger-bisect-2026-11-11"
rm -rf "$LB"
cp -a "$ROOT/.agents/ledger-bisect-2026-11-11" "$LB"
WT="/tmp/ledger-bisect-wt-${COMMIT:0:12}"
rm -rf "$WT"
git -C "$ROOT" worktree add -f "$WT" "$COMMIT" >/dev/null
export OMEGA_REPO="$WT"
cd "$WT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
TDIR="$ROOT/target/verify-${COMMIT:0:12}"
mkdir -p "$TDIR"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" nice -n 19 mbx build -p omega < /dev/null >/dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TDIR" nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null
export PIN_OBS="$LB/pin_full.txt"
python3 "$LB/tools/prepare_bisect_pin.py" "$WITNESS" >/dev/null
export BISECT_WITNESS="$WITNESS"
RUNNER=$(ls -t "$TDIR"/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$WT" OMEGA_LIBRARY_ROOT="$WT/source/library"
witness_rc() {
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
  rm -f /tmp/bisect_one_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  python3 "$LB/tools/bisect_test.py"
}
RC1=0; witness_rc || RC1=$?
RC2=0; witness_rc || RC2=$?
RC3=0; witness_rc || RC3=$?
FILTER=$(IFS=,; echo "${MEMBERS[*]}")
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/verify_members_record.txt
rm -f /tmp/verify_members_record.txt
"$LB/run_pin_corpus.sh" "$FILTER" /tmp/verify_members_record.txt >/dev/null 2>&1 || true
echo "commit=$COMMIT"
echo "witness_triplet=$RC1 $RC2 $RC3"
PIN_OBS="$LB/pin_full.txt" OMEGA_REPO="$WT" python3 "$LB/tools/verify_members.py" /tmp/verify_members_record.txt "${MEMBERS[@]}"
git -C "$ROOT" worktree remove -f "$WT" 2>/dev/null || rm -rf "$WT"
