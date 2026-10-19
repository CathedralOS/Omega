#!/bin/bash
# Honest wave status. A lane is LANDED when its fact has golden rows on main
# (the recorded row is the measurement). IN-FLIGHT when it has own commits,
# dirty files, or a live process. STALLED otherwise. LANDED detection reads the
# golden, so it never mistakes a worktree sitting at the wave base for landed work.
cd "$(dirname "$0")/../.." || exit 1
FACTS=${1:-build/swarm/live-facts.txt}
git fetch origin -q 2>/dev/null
NAT=$(git show origin/main:tests/omega/corpus_native_macos_arm64.txt 2>/dev/null)
CO=$(git show origin/main:tests/omega/corpus_outcomes.txt 2>/dev/null)
landed=0; flight=0; stalled=0; stall_list=""
while read -r fact; do
  [ -n "$fact" ] || continue
  rows=$(printf '%s\n' "$NAT" | grep -c "$fact" | head -1)
  if [ "${rows:-0}" -gt 0 ]; then
    landed=$((landed+1)); printf '  LANDED    %-38s rows=%s\n' "$fact" "$rows"; continue
  fi
  found=0
  for s in .codex/worktrees/slot-* .codex/worktrees/w2*; do
    [ -d "$s" ] || continue
    if grep -rqs "$fact" "$s/samples/cli/facts/$fact" 2>/dev/null || git -C "$s" log --oneline origin/main..HEAD 2>/dev/null | grep -q "$(echo "$fact" | tr '_' ' ' | cut -c1-24)"; then
      own=$(git -C "$s" log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
      touched=$(find "$s/samples/cli/facts/$fact" -newermt '-25 minutes' -type f 2>/dev/null | wc -l | tr -d ' ')
      name=${s##*/}
      procs=$(ps aux | grep -c "$name/" | head -1)
      if [ "$own" -gt 0 ] || [ "$touched" -gt 0 ] || [ "$procs" -gt 0 ]; then found=1; fi
    fi
  done
  if [ "$found" = "1" ]; then flight=$((flight+1)); printf '  IN-FLIGHT %-38s\n' "$fact"
  else stalled=$((stalled+1)); stall_list="$stall_list $fact"; printf '  STALLED   %-38s <-- needs attention\n' "$fact"; fi
done < "$FACTS"
cpu=$(ps aux | grep -E "[r]ustc|[m]bx" | awk '{c+=$3} END {printf "%.0f", c}')
echo "  ----"
echo "  landed=$landed in_flight=$flight stalled=$stalled cpu=${cpu:-0}%  main=$(git rev-parse --short origin/main)"
echo "  golden pass rows on main: $(printf '%s\n' "$NAT" | grep -cE '^pass/' | head -1)"
