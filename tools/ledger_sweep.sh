#!/bin/sh
# Re-record the corpus ledger at the checked-out commit, one shard at a time.
#
#   usage: sh tools/ledger_sweep.sh <logdir> <N> [jobs]
#
# Run it from the repository root of a clean checkout of the commit to measure.
# Shards count from zero. Without <logdir>/todo.txt it takes all N; with one it
# takes the shards listed there (one per line) in order, and reads the list
# again before every shard, so work can be moved between hosts while it runs.
# A finished shard goes to done.txt and a failed one to failed.txt; both are
# skipped on a restart. Creating <logdir>/stop ends the loop after the current
# shard. Every shard records into tests/omega/corpus_outcomes.txt, which the
# stamp ignores, so all shards carry the same clean commit.
#
# The numbers below were measured on the 16-core, 11 to 12 GB test VMs on
# 2026-11-09. Do not change them on a run that is already going: every change
# made mid-run that night cost a shard.
#
# - N=40 gives about 440 fixtures a shard. Each invocation spends about six
#   minutes rebuilding and verifying the runner before it checks anything, so
#   small shards are mostly overhead (N=120 was). xtask also stops one runner
#   at 1800 seconds, and 440 fixtures take 15 to 17 minutes, so much larger
#   shards do not fit.
# - jobs=6. Eight workers took the runner to 8 GB with no gain in speed, and
#   the kernel killed it.
# - The per-fixture time cap stays at the runner's default. Giving each fixture
#   three times its recorded time let a few stuck fixtures hold their workers
#   for up to 24 minutes, and a shard with several of them ran past the
#   1800-second limit with half its fixtures unchecked.
# - Start it through the machine's one-heavy-job guard where there is one
#   (tools/build_slot.md), so nothing else builds on the host meanwhile.
LOG=$1; N=$2; JOBS=${3:-6}
[ -n "$LOG" ] && [ -n "$N" ] || { echo "usage: sh tools/ledger_sweep.sh <logdir> <N> [jobs]" >&2; exit 2; }
mkdir -p "$LOG"
if [ ! -f "$LOG/todo.txt" ]; then
  k=0; while [ "$k" -lt "$N" ]; do echo "$k"; k=$((k + 1)); done > "$LOG/todo.txt"
fi
touch "$LOG/done.txt" "$LOG/failed.txt"
export OMEGA_CORPUS_JOBS="$JOBS"
while [ ! -f "$LOG/stop" ]; do
  k=""
  for c in $(cat "$LOG/todo.txt"); do
    if ! grep -q "^$c\$" "$LOG/done.txt" && ! grep -q "^$c\$" "$LOG/failed.txt"; then k=$c; break; fi
  done
  [ -z "$k" ] && break
  start=$(date +%s)
  mbx xtask corpus --sweep --filter pass/,fail/,run/ --shard "$k/$N" --record < /dev/null > "$LOG/shard-$k.log" 2>&1
  rc=$?
  echo "shard $k/$N exit $rc secs $(( $(date +%s) - start )) at $(date +%H:%M) head $(git rev-parse --short=12 HEAD)" >> "$LOG/progress.txt"
  if [ "$rc" -eq 0 ]; then echo "$k" >> "$LOG/done.txt"; else echo "$k" >> "$LOG/failed.txt"; fi
done
echo "loop ended at $(date +%H:%M)" >> "$LOG/progress.txt"
