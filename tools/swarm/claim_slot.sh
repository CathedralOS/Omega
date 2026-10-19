#!/bin/bash
# Take a verifiably-exclusive slot and RESERVE it so no second lane can be sent there.
#
# Why the plain free-slot check was insufficient: it is a POINT-IN-TIME sample. A
# lane that has not yet committed looks identical to a free slot, and a lane that
# commits seconds after the check turns a "free" slot into a shared one. A batch-4
# lane observed HEAD move c5406aa -> aa7d93e under it, with origin/main..HEAD going
# 0 -> 1 -> 2 while it worked. The check was correct and still insufficient.
#
# This reserves the slot with an O_EXCL lock file naming the fact, and REFUSES a
# slot that is already reserved. Locks record the owner and expire, so a dead lane
# cannot block a slot forever.
#
# Usage: bash tools/swarm/claim_slot.sh <fact-or-label> [first] [last]
#        bash tools/swarm/claim_slot.sh --status
#        bash tools/swarm/claim_slot.sh --release <slot>
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
LOCKDIR=build/swarm/slot-locks
mkdir -p "$LOCKDIR"
LOCK_TTL_SECONDS=${LOCK_TTL_SECONDS:-14400}   # 4h

if [ "${1:-}" = "--status" ]; then
  for f in "$LOCKDIR"/slot-*.lock; do
    [ -f "$f" ] || continue
    age=$(( $(date +%s) - $(stat -f %m "$f" 2>/dev/null || echo 0) ))
    printf '  %-10s %-40s age=%ss\n' "$(basename "$f" .lock)" "$(cat "$f")" "$age"
  done
  exit 0
fi

if [ "${1:-}" = "--release" ]; then
  rm -f "$LOCKDIR/slot-$2.lock" && echo "released slot-$2"
  exit 0
fi

label=${1:?usage: claim_slot.sh <label> [first] [last]}
first=${2:-1}; last=${3:-16}
git fetch origin -q 2>/dev/null

stale_lock() {  # $1 = slot number
  local f="$LOCKDIR/slot-$1.lock"
  [ -f "$f" ] || return 0
  local age=$(( $(date +%s) - $(stat -f %m "$f" 2>/dev/null || echo 0) ))
  [ "$age" -gt "$LOCK_TTL_SECONDS" ]
}

for i in $(seq "$first" "$last"); do
  s=".codex/worktrees/slot-$i"
  [ -d "$s" ] || continue
  if [ -f "$LOCKDIR/slot-$i.lock" ] && ! stale_lock "$i"; then
    continue
  fi
  own=$(git -C "$s" log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
  dirty=$(git -C "$s" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  procs=$(ps aux | grep -c "[s]lot-$i/" | head -1)
  if [ "$own" != "0" ] || [ "$dirty" != "0" ] || [ "$procs" != "0" ]; then
    continue
  fi
  # Reserve atomically. A racing claimer loses here rather than sharing a worktree.
  if ( set -o noclobber; printf '%s' "$label" > "$LOCKDIR/slot-$i.lock" ) 2>/dev/null; then
    echo "$i"
    exit 0
  fi
done
echo "NONE" >&2
exit 1
