#!/bin/bash
# Run a corpus fixture-pinning --record with the guards that diag_backfill.sh has but a
# DIRECT `corpus_gate --record` does not.
#
# Why this exists: a lane recorded fixtures with a direct
#     python3 tools/corpus_gate.py --native --filter <fact> --record
# and the outcomes leg wrote NINE `timeout` rows, because its fixtures need ~110s under
# load and the default per-fixture cap is 120s. The timeout guard lives in
# diag_backfill.sh only, so a direct record had no protection. Naively committing would
# have pinned nine bogus timeouts as evidence.
#
# Guards applied here:
#   - re-sync both goldens from origin/main first (avoids base-staleness LOST reports)
#   - refuse while load is above the cap, or free memory is below the floor
#   - raise the per-fixture ceiling so a loaded host does not produce timeouts
#   - after the record, REJECT it if any `timeout` row appears, or if rows were lost
#
# Usage: bash tools/swarm/safe_record.sh <filter> [--native] [--check]
#        bash tools/swarm/safe_record.sh --help
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1

if [ "${1:-}" = "--help" ] || [ $# -lt 1 ]; then
  sed -n '2,20p' "$0"
  exit 0
fi

filter=$1; shift
legs=""
for a in "$@"; do
  case "$a" in
    --native) legs="$legs --native" ;;
    --check)  legs="$legs" ;;
  esac
done
[ -n "$legs" ] || legs="--native"

LOCKDIR=build/swarm/record-slots
mkdir -p "$LOCKDIR"

git fetch origin -q 2>/dev/null
echo "== operating on: $(pwd) =="
if [ "$(pwd -P)" = "/Volumes/Crucial/CathedralOS/Omega" ]; then
  echo "   ABORT: this is the MAIN CHECKOUT; its working tree may carry an unlanded" >&2
  echo "   migration. Run this from your own worktree." >&2
  exit 2
fi

# Contamination gate: a tree rewritten without its parser half records parse errors.
contam=$(git status --porcelain 2>/dev/null | grep -c '\.omg$')
if [ "${contam:-0}" -gt 0 ]; then
  echo "   ABORT: ${contam} modified .omg files in this tree. If you did not edit them," >&2
  echo "   this is contamination; stash them before recording." >&2
  exit 2
fi

load=$(sysctl -n vm.loadavg 2>/dev/null | awk '{print int($2)}')
ps_bytes=$(vm_stat 2>/dev/null | awk '/page size of/{print $8}')
free_mb=$(vm_stat 2>/dev/null | awk -v ps="${ps_bytes:-16384}" '/Pages free/{gsub(/\./,"",$3); printf "%d", int(int($3)*ps/1048576)}')
if [ "${load:-0}" -gt 60 ]; then
  echo "   ABORT: load ${load} too high to record (timeout rows). Retry when load < 60." >&2
  exit 1
fi
if [ "${free_mb:-0}" -lt 512 ]; then
  echo "   ABORT: only ${free_mb} MB free (page size ${ps_bytes}). Retry when >= 512 MB." >&2
  exit 1
fi

# Serialize records host-wide so lanes do not multiply compiles into an OOM.
RECORD_SLOTS=${RECORD_SLOTS:-3}
acquired=""
for attempt in $(seq 1 120); do
  for s in $(seq 0 $((RECORD_SLOTS-1))); do
    if ( set -o noclobber; printf '%s' "$$" > "$LOCKDIR.record$s" ) 2>/dev/null; then acquired="$LOCKDIR.record$s"; break; fi
    owner=$(cat "$LOCKDIR.record$s" 2>/dev/null)
    [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null && rm -f "$LOCKDIR.record$s"
  done
  [ -n "$acquired" ] && break
  echo "   waiting for a record slot (${attempt}/120)..."
  sleep 5
done
[ -n "$acquired" ] || { echo "   ABORT: no record slot; retry later." >&2; exit 1; }
recording=""
on_exit() {
  rm -f "$acquired"
  if [ -n "${recording:-}" ]; then
    for f in tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt; do
      git checkout HEAD -- "$f" 2>/dev/null
      git reset -q HEAD -- "$f" 2>/dev/null
    done
    echo "   (interrupted: restored goldens from HEAD)" >&2
  fi
}
trap on_exit EXIT INT TERM HUP

echo "== filter: $filter | legs:${legs} | load ${load} | free ${free_mb}MB =="
# Raise the per-fixture ceiling: fixtures can need ~110s under load and the default 120s
# produces timeouts that look like recorded evidence.
recording=1
OMEGA_CORPUS_JOBS=${OMEGA_CORPUS_JOBS:-3} OMEGA_CORPUS_FIXTURE_SECS=${OMEGA_CORPUS_FIXTURE_SECS:-900} \
  python3 tools/corpus_gate.py --filter "$filter" $legs --record 2>&1 | tail -5

recording=""
rm -f "$acquired"; trap - EXIT INT TERM HUP

bad=0
for f in tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt; do
  [ -f "$f" ] || continue
  t=$(awk '/timeout/{n++} END{print n+0}' "$f")
  if [ "$t" != "0" ]; then
    echo "   REJECT: ${t} timeout row(s) in ${f##*/}. Those record host load, not evidence." >&2
    bad=1
  fi
done
lost=$(python3 - <<'PY'
import subprocess, pathlib
tot = 0
for f in ['tests/omega/corpus_outcomes.txt','tests/omega/corpus_native_macos_arm64.txt']:
    t = subprocess.run(['git','show',f'origin/main:{f}'],capture_output=True,text=True).stdout
    m = {l.split()[0] for l in t.splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
    p = pathlib.Path(f)
    if not p.exists(): continue
    n = {l.split()[0] for l in p.read_text().splitlines() if l.strip() and not l.startswith('#') and not l[0].isspace()}
    tot += len(m - n)
print(tot)
PY
)
if [ "${lost:-0}" != "0" ]; then
  echo "   LOST=${lost} vs origin/main. If those rows are OTHER facts and your own diff has" >&2
  echo "   zero deletions, this is BASE STALENESS: rebase and re-run, do not re-add rows." >&2
  bad=1
fi

if [ "$bad" != "0" ]; then
  echo "   Record NOT accepted. Inspect before committing." >&2
  exit 1
fi
echo "   OK: no timeout rows, LOST 0. Commit per fact."
