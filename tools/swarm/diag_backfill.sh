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

LOCKDIR=build/swarm/record-slots
REC_LOG=${REC_LOG:-/tmp/diag-record-$$.log}
mkdir -p "$(dirname "$LOCKDIR")"
git fetch origin -q 2>/dev/null
# Sync ONCE, before the loop. Syncing per fact would `git checkout origin/main --`
# the goldens at the start of every iteration, silently discarding the recording the
# previous iteration just made: a lane recording nine facts would land only the LAST
# one. A backfill lane hit exactly this (its case_equality rows went back to 0/10 in
# the working tree while recording guarded_line_loan; the commit was safe, the tree
# was not).
echo "== operating on: $(pwd) =="
# Capture every fact's diagnostic count BEFORE touching the goldens. A diagnostic can
# vanish while its row KEY survives, so the LOST check cannot see it: a lane's second
# fact re-synced the golden to main, which reverted its FIRST fact's diagnostics, and
# `git add -A` committed that revert under the second fact's message. Nothing reported
# it. Compare before/after and fail loudly on any regression.
PRE_COUNTS=$(mktemp)
# Refreshed inside the loop before EACH record. Templating it once and deleting it
# per fact (as the earlier cleanup did) meant facts 2..N died with FileNotFoundError
# inside the regression check, so the guard silently did not run for them -- exactly
# the facts a multi-fact run is most likely to lose.
SNAPSHOT_DIAGS() { python3 tools/swarm/diag_counts.py tests/omega/corpus_outcomes.txt > "$PRE_COUNTS" 2>/dev/null || true; }
DIAG_REGRESSION() {
  python3 - "$PRE_COUNTS" <<'PYEOF'
import subprocess, sys
from pathlib import Path
sys.path.insert(0, 'tools/swarm')
import importlib.util
spec = importlib.util.spec_from_file_location('dc', 'tools/swarm/diag_counts.py')
dc = importlib.util.module_from_spec(spec); spec.loader.exec_module(dc)
pre = {}
for line in Path(sys.argv[1]).read_text().splitlines():
    f, c = line.rsplit(' ', 1)
    wd, tot = c.split('/')
    pre[f] = (int(wd), int(tot))
now = dc.counts('tests/omega/corpus_outcomes.txt')
regressed = [f for f, (wd, tot) in pre.items()
             if wd > 0 and now.get(f, (0, 0))[0] < wd]
if regressed:
    print("REGRESSED " + " ".join(f"{f}:{pre[f][0]}->{now.get(f,(0,0))[0]}" for f in sorted(regressed)))
PYEOF
}
if [ "$(pwd -P)" = "/Volumes/Crucial/CathedralOS/Omega" ]; then
  echo "   ABORT: this is the MAIN CHECKOUT. This tool acts on the tree it lives in," >&2
  echo "   and the main checkout working tree carries an unlanded migration. Copy the" >&2
  echo "   tool into your own worktree and run it there." >&2
  exit 2
fi
echo "== re-sync goldens from origin/main (once) =="
# `git checkout <ref> -- <path>` stages the file as well as writing it, so a later
# abort left a STAGED golden diff the lane never authored. Restore the working tree and
# unstage these two paths, so the index is exactly as the lane left it.
# MERGE origin/main's rows into the golden; do not overwrite it. An overwrite lost a
# lane's already-committed fact A when it ran the tool for fact B, because main did not
# yet carry A's diagnostics -- and `git add -A` then committed that revert under B's
# message. Merging keeps every row present on either side, preferring the side that
# carries a diagnostic, so a run can never un-record an earlier fact.
for g in tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt; do
  python3 tools/swarm/merge_rows.py "$g" origin/main >/dev/null 2>&1 \
    || git restore --source=origin/main --worktree "$g" 2>/dev/null \
    || git checkout origin/main -- "$g" 2>/dev/null
  git reset -q HEAD -- "$g" 2>/dev/null
done

for fact in "$@"; do
  echo "== $fact =="
  SNAPSHOT_DIAGS
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
  # A corpus record compiles fixtures in PARALLEL, and `build_slot.py` gates only
  # `mbx build` -- it does not govern this. With sixteen lanes recording at once the
  # host ran 11 corpus_runner processes and 23 rustc on 10 cores, dropped to 27 MB of
  # free RAM, and the kernel logged 4264 kills in 30 minutes. One lane's record was
  # OOM-killed mid-run.
  #
  # Gate on FREE MEMORY as well as load, and take a global record slot so records
  # serialize across lanes. Held only for the duration of this fact's record.
  # Integerize INSIDE awk. `print int($3)*4096/1048576` is awk floating-point
  # division and printed e.g. "85.3828"; `[ "$free_mb" -lt 512 ]` then errored
  # ("integer expression expected") and evaluated FALSE, so this guard always took
  # the else branch and never fired -- inert exactly when it was needed. awk's int()
  # returns an integer-formatted number, and a belt-and-braces truncation follows.
  # Read the page size the kernel REPORTS rather than assuming 4096: this host's
  # vm_stat header says "page size of 16384 bytes", so a hardcoded 4096 reported
  # 17 MB when 75 MB was free -- a 4x under-report that refused records which were
  # actually safe. awk's int() keeps the value integral, and a shell truncation
  # follows so `[ -lt ]` cannot receive a fractional operand (an earlier version
  # printed "85.3828" and the comparison silently evaluated FALSE).
  free_mb=$(vm_stat 2>/dev/null | awk '/page size of/{ps=$8} /Pages free/{gsub(/\./,"",$3); printf "%d", int(int($3)*ps/1048576)}')
  free_mb=${free_mb%%.*}
  ps_bytes=$(vm_stat 2>/dev/null | awk '/page size of/{print $8}')
  if [ "${free_mb:-999}" -lt 1200 ]; then
    echo "   ABORT: only ${free_mb} MB free (floor 1.2 GB); a record here risks an OOM kill. Retry later."
    continue
  fi
  # Serialize records host-wide: at most RECORD_SLOTS concurrent.
  # Default 1: with ~16 lanes live the host oscillated between 74 MB and 1050 MB
# free, two records were OOM-killed mid-run (jetsam entries in the log at the
# same second), and a brief pass of the memory floor did not prevent the kill --
# memory collapses DURING the run. Serializing to one record at a time is the
# only setting that has kept a record alive. Raise only with a measured reason.
RECORD_SLOTS=${RECORD_SLOTS:-1}
  acquired=""
  for attempt in $(seq 1 120); do
    for s in $(seq 0 $((RECORD_SLOTS-1))); do
      if ( set -o noclobber; printf '%s' "$$" > "$LOCKDIR.record$s" ) 2>/dev/null; then
        acquired="$LOCKDIR.record$s"; break
      fi
      # reclaim a slot whose owner is gone
      owner=$(cat "$LOCKDIR.record$s" 2>/dev/null)
      [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null && rm -f "$LOCKDIR.record$s"
    done
    [ -n "$acquired" ] && break
    echo "   waiting for a record slot (${attempt}/120)..."
    sleep 5
  done
  if [ -z "$acquired" ]; then
    echo "   ABORT: no record slot available; retry later."
    continue
  fi
  # EXIT alone does not fire on SIGTERM/SIGINT/HUP. A lane's recorder was SIGTERM'd
  # mid-run and the revert never happened, leaving a PARTIAL golden staged (LOST=84).
  on_exit() {
    rm -f "$acquired"
    # Kill the recorder's process group so it cannot keep writing after we exit.
    if [ -n "${recorder_pid:-}" ] && kill -0 "$recorder_pid" 2>/dev/null; then
      kill -TERM -"$recorder_pid" 2>/dev/null || kill -TERM "$recorder_pid" 2>/dev/null
      sleep 2
      kill -KILL -"$recorder_pid" 2>/dev/null || kill -KILL "$recorder_pid" 2>/dev/null
      echo "   (interrupted: killed the recorder process group $recorder_pid)" >&2
    fi
    if [ -n "${recording:-}" ]; then
      # Restore ONLY this fact's row blocks. A whole-file `git checkout HEAD --` cannot
      # preserve this run's earlier facts: after a rebase HEAD holds MAIN's golden,
      # where an earlier fact is still bare, so reverting one bad fact would discard
      # every earlier fact's diagnostic and LOST would not notice (the key survives).
      python3 tools/swarm/revert_block.py tests/omega/corpus_outcomes.txt HEAD "$fact" >/dev/null 2>&1
      git reset -q HEAD -- tests/omega/corpus_outcomes.txt 2>/dev/null
      echo "   (interrupted: restored the outcomes golden from HEAD)" >&2
    fi
  }
  recording=""
  trap on_exit EXIT INT TERM HUP
  echo "   load ${load}; recording with jobs=3"
  recording=1
  # Run the recorder in its OWN PROCESS GROUP and remember it, so a signal to this
  # wrapper kills the recorder too. Without this, SIGTERM to the wrapper orphans the
  # python3 corpus_gate child; the orphan keeps running and writes the golden against
  # whatever base it re-synced at start, which may be stale by then. A lane hit this
  # twice: the orphan's write silently reverted already-COMMITTED diagnostics to bare
  # `rejected` (0/9) while the lane believed the record had been aborted, and the
  # wrapper's post-record verification never ran so nothing caught it.
  set -m 2>/dev/null || true
  OMEGA_CORPUS_JOBS=3 OMEGA_CORPUS_FIXTURE_SECS=600 \
    python3 tools/corpus_gate.py --filter "$fact" --record >"$REC_LOG" 2>&1 &
  recorder_pid=$!
  set +m 2>/dev/null || true
  wait "$recorder_pid"
  out=$(cat "$REC_LOG")
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
  recording=""
  rm -f "$acquired" 2>/dev/null; trap - EXIT INT TERM HUP
  # LOST and timeout counts CANNOT detect a source tree that does not compile: no row
  # is lost and no timeout is written, yet the rows recorded are parse errors rather
  # than the fact's obligation. Observed with a contaminated tree:
  #   rejected 6ms unexpected
  #     source/library/core/code_layout.omg:3:1: expected one of ..., found `public`
  # where the real row is `rejected ~1700ms expected` naming the fact's obligation.
  # Three independent signatures, any one of which catches it.
  bad=0
  # Did any ALREADY-diagnosed fact lose its diagnostic? That is silent data loss.
  reg=$(DIAG_REGRESSION)
  if [ -n "$reg" ]; then
    echo "   ABORT: earlier facts lost their diagnostics: ${reg#REGRESSED }" >&2
    echo "   (the row keys survive, so LOST cannot see this)" >&2
    bad=1
  fi
  if grep -qE "^fail/facts/${fact}[^ ]* rejected [0-9]+ms unexpected" tests/omega/corpus_outcomes.txt 2>/dev/null; then
    echo "   ABORT: a row for $fact is marked 'unexpected' (not the authored rejection)."
    bad=1
  fi
  if grep -A1 -E "^fail/facts/${fact}[^ ]* rejected" tests/omega/corpus_outcomes.txt 2>/dev/null \
       | grep -qE "expected one of|found identifier|source/library/.*\.omg:[0-9]+"; then
    echo "   ABORT: a recorded diagnostic is a PARSE error, not the fact's obligation."
    bad=1
  fi
  fast=$(awk -v f="$fact" '$1 ~ "^fail/facts/"f && $3 == "rejected" { }
       $1 ~ "^fail/facts/"f && /rejected/ { if (match($0, /rejected ([0-9]+)ms/, m)) if (m[1] < 200) n++ } END { print n+0 }' tests/omega/corpus_outcomes.txt 2>/dev/null)
  if [ "${fast:-0}" != "0" ]; then
    echo "   ABORT: ${fast} row(s) rejected in under 200ms; real check diagnostics take far longer."
    bad=1
  fi
  if [ "$bad" != "0" ]; then
    echo "   REVERTING this fact (record is not the fact's obligation); earlier facts preserved"
    # Restore ONLY this fact's row blocks. A whole-file `git checkout HEAD --` cannot
      # preserve this run's earlier facts: after a rebase HEAD holds MAIN's golden,
      # where an earlier fact is still bare, so reverting one bad fact would discard
      # every earlier fact's diagnostic and LOST would not notice (the key survives).
      python3 tools/swarm/revert_block.py tests/omega/corpus_outcomes.txt HEAD "$fact" >/dev/null 2>&1
    continue
  fi
  if [ "$after" != "0" ] || [ "$timeouts" != "0" ]; then
    # Restore only THIS fact's rows from HEAD, so a bad record cannot carry a loss
    # forward while earlier facts in the same run keep their diagnostics.
    echo "   REVERTING this fact (LOST=$after timeouts=$timeouts); earlier facts preserved"
    # Restore ONLY this fact's row blocks. A whole-file `git checkout HEAD --` cannot
      # preserve this run's earlier facts: after a rebase HEAD holds MAIN's golden,
      # where an earlier fact is still bare, so reverting one bad fact would discard
      # every earlier fact's diagnostic and LOST would not notice (the key survives).
      python3 tools/swarm/revert_block.py tests/omega/corpus_outcomes.txt HEAD "$fact" >/dev/null 2>&1
  else
    echo "   OK: no rows lost, no timeouts"
    echo "   COMMIT THIS FACT NOW so the next iteration cannot affect it:"
    echo "     git add -A && git commit -m 'tests: record $fact reject diagnostics'"
  fi
done

# One cleanup after the whole loop, not per fact.
rm -f "$PRE_COUNTS" 2>/dev/null || true
