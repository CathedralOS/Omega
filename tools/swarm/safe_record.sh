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
# Usage: bash tools/swarm/safe_record.sh <filter> [--native | --check]
#        (no flag records the native leg; --check records the check leg, whose
#         fail-tier reject diagnostics live in tests/omega/corpus_outcomes.txt)
#        bash tools/swarm/safe_record.sh --help
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1

if [ "${1:-}" = "--help" ] || [ $# -lt 1 ]; then
  sed -n '2,20p' "$0"
  exit 0
fi

filter=$1; shift
# Leg selection. The check leg is the one selected by NO leg flag, but a bare
# `legs=""` was then forced to `--native`, so `--check` recorded the native
# golden -- which carries no fail-tier rows at all -- and could never pin a
# check-leg reject diagnostic. Keep the historical no-flag default (native) and
# let `--check` select the check leg explicitly.
legs=""
native=0
check=0
for a in "$@"; do
  case "$a" in
    --native) native=1 ;;
    --check)  check=1 ;;
  esac
done
if [ "$native" = 1 ] || [ "$check" = 0 ]; then legs="--native"; fi

# ONE lock scheme, host-wide, keyed by the CHECKOUT: the slot files live inside
# <checkout>/build/swarm/record-slots and every worktree of this checkout resolves the
# same directory, the way tools/build_slot.py's slot_directory does it. Two earlier
# defects made the cap imaginary. (1) This script wrote the SIBLING path
# `build/swarm/record-slots.record0`, resolved against whatever worktree ran it, while
# other lanes wrote `build/swarm/record-slots/record0`: two schemes, mutually invisible,
# so a lane could hold one while another "acquired" the other. (2) The reclaim guard
# `[ -n "$owner" ] && ! kill -0 "$owner"` could not clear an EMPTY file -- the state a
# writer killed between truncate and write leaves behind -- so one zero-byte file blocked
# every later lane for the full wait cycle.
common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)
if [ -n "$common" ]; then checkout_root=$(dirname "$common"); else checkout_root=$(pwd -P); fi
SLOT_DIR=$checkout_root/build/swarm/record-slots
mkdir -p "$SLOT_DIR"

# Take slot $1: print its path, or return 1 while another live owner holds it. A file is
# reclaimable when it is empty or its owner is gone; reclaim and takeover are separate
# exclusive creates, so a racing lane loses the slot instead of sharing it.
acquire_record_slot() {
  local file="$SLOT_DIR/record$1"
  if ( set -o noclobber; printf '%s' "$$" > "$file" ) 2>/dev/null; then
    printf '%s' "$file"; return 0
  fi
  local owner
  owner=$(cat "$file" 2>/dev/null)
  if [ -z "$owner" ] || ! kill -0 "$owner" 2>/dev/null; then
    rm -f "$file"
    if ( set -o noclobber; printf '%s' "$$" > "$file" ) 2>/dev/null; then
      printf '%s' "$file"; return 0
    fi
  fi
  return 1
}

git fetch origin -q 2>/dev/null
echo "== operating on: $(pwd) =="
if [ "$(pwd -P)" = "/Volumes/Crucial/CathedralOS/Omega" ]; then
  echo "   ABORT: this is the MAIN CHECKOUT; its working tree may carry an unlanded" >&2
  echo "   migration. Run this from your own worktree." >&2
  exit 2
fi

# Contamination gate, migration-aware. The failure this exists to catch is a tree
# whose library half was reverted without its parser half: every fixture then
# records a parse error instead of its own obligation. A migration necessarily
# modifies `.omg` files, so the refusal would block honest work while catching
# nothing; the check below prints what changed and the POST-RECORD check refuses a
# record whose diagnostics are parse errors in a library source, which is the
# actual failure signature (no rule in the corpus names a library line).
contam=$(git status --porcelain 2>/dev/null | grep -c '\.omg$')
if [ "${contam:-0}" -gt 0 ]; then
  echo "   NOTE: ${contam} modified .omg file(s) in this tree; treating them as authored edits:"
  git status --porcelain | grep '\.omg$' | head -5 | sed 's/^/      /'
  [ "$contam" -le 5 ] || echo "      ... and $((contam - 5)) more"
  echo "      a record that comes back as a library parse error will be refused below."
fi

load=$(sysctl -n vm.loadavg 2>/dev/null | awk '{print int($2)}')
ps_bytes=$(vm_stat 2>/dev/null | awk '/page size of/{print $8}')
free_mb=$(vm_stat 2>/dev/null | awk -v ps="${ps_bytes:-16384}" '/Pages free/{gsub(/\./,"",$3); printf "%d", int(int($3)*ps/1048576)}')
if [ "${load:-0}" -gt 60 ]; then
  echo "   ABORT: load ${load} too high to record (timeout rows). Retry when load < 60." >&2
  exit 1
fi
if [ "${free_mb:-0}" -lt 1200 ]; then
  echo "   ABORT: only ${free_mb} MB free (page size ${ps_bytes}). Retry when >= 1.2 GB." >&2
  exit 1
fi

# Serialize records host-wide so lanes do not multiply compiles into an OOM.
# Default 1: with ~16 lanes live the host oscillated between 74 MB and 1050 MB
# free, two records were OOM-killed mid-run (jetsam entries in the log at the
# same second), and a brief pass of the memory floor did not prevent the kill --
# memory collapses DURING the run. Serializing to one record at a time is the
# only setting that has kept a record alive. Raise only with a measured reason.
RECORD_SLOTS=${RECORD_SLOTS:-1}
acquired=""
for attempt in $(seq 1 120); do
  for s in $(seq 0 $((RECORD_SLOTS-1))); do
    if acquired=$(acquire_record_slot "$s"); then break; fi
  done
  [ -n "$acquired" ] && break
  echo "   waiting for a record slot (${attempt}/120); pool ${SLOT_DIR}..."
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
# A record that measured nothing must not read as a record that measured
# everything: the gate's own status and the goldens' bytes are the only two
# facts that separate them: the gate's own status, its output naming a filter
# that matched no fixtures (which the status alone reports only as usage), and a
# byte comparison that catches a --record the gate REFUSED (a contradicted
# oracle exits 3 and writes nothing), which no status-only check would see. The
# output is captured rather than piped so both the status and that line survive.
goldens='tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt'
# Raise the per-fixture ceiling: fixtures can need ~110s under load and the default 120s
# produces timeouts that look like recorded evidence.
before=$(git hash-object $goldens 2>/dev/null | tr '\n' ' ')
recording=1
record_output=$(OMEGA_CORPUS_JOBS=${OMEGA_CORPUS_JOBS:-3} OMEGA_CORPUS_FIXTURE_SECS=${OMEGA_CORPUS_FIXTURE_SECS:-900} \
  python3 tools/corpus_gate.py --filter "$filter" $legs --record 2>&1)
gate_status=$?
printf '%s\n' "$record_output" | tail -5

recording=""
rm -f "$acquired"; trap - EXIT INT TERM HUP

bad=0
if [ "${gate_status:-0}" = 2 ] || [ "${gate_status:-0}" = 4 ]; then
  echo "   REJECT: corpus_gate exited ${gate_status} (usage or infrastructure); nothing was measured." >&2
  bad=1
fi
if printf '%s\n' "$record_output" | grep -q "matched no fixtures"; then
  echo "   REJECT: the filter matched no fixtures: ${filter}" >&2
  echo "   Name at least one fixture path fragment; a filter that selects nothing proves nothing." >&2
  bad=1
fi
after=$(git hash-object $goldens 2>/dev/null | tr '\n' ' ')
if [ "$before" = "$after" ]; then
  echo "   REJECT: neither golden changed, so this run recorded nothing (a refused or" >&2
  echo "   empty --record reads exactly like a clean one). Inspect the gate output above." >&2
  bad=1
fi
for f in tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt; do
  [ -f "$f" ] || continue
  t=$(awk '/timeout/{n++} END{print n+0}' "$f")
  if [ "$t" != "0" ]; then
    echo "   REJECT: ${t} timeout row(s) in ${f##*/}. Those record host load, not evidence." >&2
    bad=1
  fi
done
# The contamination signature: a recorded diagnostic that is a parse error in a
# LIBRARY source. No rule in the corpus names a library line, so this cannot fire
# on a fixture that legitimately pins a parse error in its own source.
parse=$(awk '/source\/library\/[^ ]*\.omg:[0-9]+/ && /expected|found|unexpected/ {n++} END{print n+0}' \
  tests/omega/corpus_outcomes.txt tests/omega/corpus_native_macos_arm64.txt 2>/dev/null)
if [ "${parse:-0}" != "0" ]; then
  echo "   REJECT: ${parse} recorded diagnostic(s) are parse errors in a library source." >&2
  echo "   That is a rewritten tree, not the fixtures' obligations. Restore the library and re-record." >&2
  bad=1
fi
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
