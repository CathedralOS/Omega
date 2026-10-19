#!/bin/bash
# Re-record the check-leg diagnostics for facts whose fail-twin rows are pinned as
# bare `rejected` with no diagnostic line.
#
# Why this matters: AGENTS.md says "A missing or unrelated reject diagnostic is not
# evidence that the fact is enforced." A bare `rejected` row proves the twin was
# refused, but NOT that it was refused for the right reason -- it could be refusing
# on an unrelated error. Recording the diagnostic turns "it rejected" into "it
# rejected for the stated obligation".
#
# Usage: bash tools/swarm/diag_backfill.sh <fact> [more facts...]
#        bash tools/swarm/diag_backfill.sh --list      (show the missing set)
#
# Run this from the lane worktree that owns the facts. It re-records with the
# golden's own calibration (jobs=3) and refuses to keep a record that lost rows.
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1

if [ "${1:-}" = "--list" ]; then
  python3 "$(dirname "$0")/diag_list.py"
  exit 0
fi

[ $# -ge 1 ] || { echo "usage: $0 <fact> [more facts...] | --list" >&2; exit 2; }

lost() {
  python3 - <<'PY'
import subprocess, sys, pathlib
f = 'tests/omega/corpus_outcomes.txt'
t = subprocess.run(['git','show',f'origin/main:{f}'],capture_output=True,text=True).stdout
m = {l.split()[0] for l in t.splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
n = {l.split()[0] for l in pathlib.Path(f).read_text().splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
print(len(m - n))
PY
}

git fetch origin -q 2>/dev/null
# Sync ONCE, before the loop. Syncing per fact would `git checkout origin/main --`
# the goldens at the start of every iteration, silently discarding the recording the
# previous iteration just made: a lane recording nine facts would land only the LAST
# one. A backfill lane hit exactly this (its case_equality rows went back to 0/10 in
# the working tree while recording guarded_line_loan; the commit was safe, the tree
# was not).
echo "== re-sync goldens from origin/main (once) =="
git checkout origin/main -- tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt 2>/dev/null
git checkout origin/main -- tests/omega/corpus_native_macos_arm64.txt 2>/dev/null

for fact in "$@"; do
  echo "== $fact =="
  before=$(lost)
  [ "$before" = "0" ] || { echo "   ABORT: pre-record LOST=$before (rebase first)"; continue; }
  echo "   pre-record LOST=0 (rows from earlier facts in this run are preserved)"
  # Refuse to record while the host is too loaded. A --record with compiles in
  # flight silently writes `timeout` rows instead of real outcomes, and those rows
  # look like recorded evidence. Two lanes lost a full record cycle to this.
  load=$(sysctl -n vm.loadavg 2>/dev/null | awk '{print int($2)}')
  if [ "${load:-0}" -gt 60 ]; then
    echo "   ABORT: load ${load} is too high to record (timeout rows). Re-run when load < 60."
    continue
  fi
  echo "   load ${load}; recording with jobs=3"
  out=$(OMEGA_CORPUS_JOBS=3 OMEGA_CORPUS_FIXTURE_SECS=600 \
    python3 tools/corpus_gate.py --filter "$fact" --record 2>&1)
  # A record is only trustworthy if no compile was still in flight when it ended.
  if echo "$out" | grep -qE '[1-9][0-9]* compile\(s\) in flight'; then
    echo "   ABORT: compiles were still in flight; that record wrote timeout rows."
    echo "   (earlier facts recorded in this run stay committed; only this one is skipped)"
    continue
  fi
  after=$(lost)
  # grep -c exits 1 when it matches nothing, so `|| echo 0` appended a SECOND
  # value and made this "0\n0", which then failed the string comparison below and
  # reverted a perfectly good record. Count with awk instead, which exits 0.
  # grep -c exits 1 on zero matches, so `|| echo 0` appended a SECOND zero and made
  # this "0\n0", which failed the comparison below and reverted good recordings. awk
  # exits 0 and is used here; the lane's suggested grep+${x:-0} form is equivalent.
  timeouts=$(awk '/timeout/{n++} END{print n+0}' tests/omega/corpus_outcomes.txt)
  echo "   post-record LOST=$after timeouts=$timeouts"
  if [ "$after" != "0" ] || [ "$timeouts" != "0" ]; then
    # Restore only THIS fact's rows from HEAD, so a bad record cannot carry a loss
    # forward while earlier facts in the same run keep their diagnostics.
    echo "   REVERTING this fact (LOST=$after timeouts=$timeouts); earlier facts preserved"
    git checkout HEAD -- tests/omega/corpus_outcomes.txt 2>/dev/null
  else
    echo "   OK: no rows lost, no timeouts"
    echo "   COMMIT THIS FACT NOW so the next iteration cannot affect it:"
    echo "     git add -A && git commit -m 'tests: record $fact reject diagnostics'"
  fi
done
