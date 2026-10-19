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
  python3 - <<'PY'
import subprocess
t = subprocess.run(['git','show','origin/main:tests/omega/corpus_outcomes.txt'],
                   capture_output=True, text=True).stdout.splitlines()
missing = {}
i = 0
while i < len(t):
    l = t[i]
    if l.startswith('fail/facts/') and 'rejected' in l:
        fact = l.split()[0].split('fail/facts/')[1].split('_through_')[0].split('_unproved')[0]
        j = i + 1; has = False
        while j < len(t) and (t[j].startswith('\t') or t[j].startswith('    ')) and t[j].strip():
            has = True; j += 1
        if not has:
            missing.setdefault(fact, 0)
            missing[fact] += 1
        i = j
    else:
        i += 1
print(f"{len(missing)} facts, {sum(missing.values())} rows missing diagnostics")
for f, c in sorted(missing.items()):
    print(f"  {c:3}  {f}")
PY
  exit 0
fi

[ $# -ge 1 ] || { echo "usage: $0 <fact> [more facts...] | --list" >&2; exit 2; }

lost() {
  python3 - "$1" <<'PY'
import subprocess, sys, pathlib
f = 'tests/omega/corpus_outcomes.txt'
t = subprocess.run(['git','show',f'origin/main:{f}'],capture_output=True,text=True).stdout
m = {l.split()[0] for l in t.splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
n = {l.split()[0] for l in pathlib.Path(f).read_text().splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
print(len(m - n))
PY
}

git fetch origin -q 2>/dev/null
for fact in "$@"; do
  echo "== $fact: re-sync goldens from origin/main =="
  git checkout origin/main -- tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt 2>/dev/null
  before=$(lost)
  [ "$before" = "0" ] || { echo "   ABORT: pre-record LOST=$before (rebase first)"; continue; }
  echo "   pre-record LOST=0; recording with jobs=3"
  OMEGA_CORPUS_JOBS=3 OMEGA_CORPUS_FIXTURE_SECS=600 \
    python3 tools/corpus_gate.py --filter "$fact" --record >/dev/null 2>&1
  after=$(lost)
  timeouts=$(grep -c 'timeout' tests/omega/corpus_outcomes.txt 2>/dev/null || echo 0)
  echo "   post-record LOST=$after timeouts=$timeouts"
  if [ "$after" != "0" ] || [ "$timeouts" != "0" ]; then
    echo "   REVERTING: record lost rows or wrote timeout rows"
    git checkout origin/main -- tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt
  else
    echo "   OK: no rows lost, no timeouts"
  fi
done
