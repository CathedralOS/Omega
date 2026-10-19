#!/bin/bash
# Report which pool slots are genuinely free before dispatching a new agent.
# A slot is FREE only if it has no own commits ahead of main AND no dirty files
# AND no live process. Anything else is owned by an in-flight agent.
# Usage: bash tools/swarm/free_slots.sh [first] [last]
cd "$(dirname "$0")/../.." || exit 1
first=${1:-1}; last=${2:-16}
git fetch origin -q 2>/dev/null
for i in $(seq "$first" "$last"); do
  s=".codex/worktrees/slot-$i"
  [ -d "$s" ] || { echo "slot-$i MISSING"; continue; }
  own=$(git -C "$s" log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
  dirty=$(git -C "$s" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
  procs=$(ps aux | grep -c "[s]lot-$i/" | head -1)
  if [ "$own" = "0" ] && [ "$dirty" = "0" ] && [ "$procs" = "0" ]; then
    echo "slot-$i FREE"
  else
    echo "slot-$i BUSY own=$own dirty=$dirty procs=$procs"
  fi
done
