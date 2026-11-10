#!/bin/sh
# One heavy job per VM. Stacked jobs kept killing the ledger re-record on
# 2026-11-09 (an out-of-memory kill and a blown 30-minute runner limit), so
# every build, corpus run or compiler batch on this machine goes through:
#
#   vm-heavy <label> <command...>
#
# It refuses with exit 75 while a compiler build, a corpus run or another
# guarded job is active here. Otherwise it runs the command holding the lock.
# A job killed with SIGKILL leaves the lock behind: after checking that nothing
# is running, remove ~/omega-agent/heavy.lock by hand.
A=$HOME/omega-agent; L=$A/heavy.lock
if command -v tasklist > /dev/null 2>&1; then
  busy=$(tasklist 2> /dev/null | grep -i -E "^(cargo|rustc|xtask|corpus_runner)" | head -1 | cut -c1-40)
else
  busy=$(ps -eo comm= | grep -E "^(cargo|rustc|xtask|corpus_runner)" | head -1)
fi
if [ -n "$busy" ]; then
  echo "vm-heavy: refused, this machine is busy: $busy" >&2
  exit 75
fi
if ! mkdir "$L" 2> /dev/null; then
  echo "vm-heavy: refused, lock held by: $(cat "$L/owner" 2> /dev/null)" >&2
  exit 75
fi
label=$1; shift
echo "$label, since $(date '+%m-%d %H:%M')" > "$L/owner"
trap 'rm -rf "$L"' EXIT
trap 'exit 130' HUP INT TERM
"$@"
