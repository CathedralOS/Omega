#!/bin/bash
# Allocate a worktree slot that is verifiably exclusive, then print it.
# Refuses to return a slot that has own commits, dirty files, or a live process.
# Usage: bash tools/swarm/assign_slot.sh [first] [last]
cd "$(dirname "$0")/../.." || exit 1
first=${1:-1}; last=${2:-16}
git fetch origin -q 2>/dev/null
for i in $(seq "$first" "$last"); do
  s=".codex/worktrees/slot-$i"
  [ -d "$s" ] || continue
  own=$(git -C "$s" log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
  dirty=$(git -C "$s" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  procs=$(ps aux | grep -c "[s]lot-$i/" | head -1)
  if [ "$own" = "0" ] && [ "$dirty" = "0" ] && [ "$procs" = "0" ]; then
    echo "$i"
    exit 0
  fi
done
echo "NONE" >&2
exit 1
