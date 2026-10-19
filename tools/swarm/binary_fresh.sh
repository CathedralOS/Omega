#!/bin/bash
# Is the compiler binary in this worktree fresh relative to HEAD?
# Worktrees make `.git` a FILE, so `find <bin> -newer .git/HEAD` errors and the
# `||` branch falsely reports STALE. Resolve the real git dir instead.
# Usage: bash tools/swarm/binary_fresh.sh [path-to-binary]
bin=${1:-target/debug/omega}
if [ ! -f "$bin" ]; then echo "MISSING: $bin does not exist (rebuild; never report exit 127 as a result)"; exit 2; fi
gitdir=$(git rev-parse --git-dir 2>/dev/null) || { echo "ERROR: not in a git worktree"; exit 3; }
headfile="$gitdir/HEAD"
if [ ! -f "$headfile" ]; then echo "ERROR: no HEAD at $headfile"; exit 3; fi
if [ "$bin" -nt "$headfile" ]; then echo "FRESH: $bin is newer than HEAD"; exit 0; fi
echo "STALE: $bin predates HEAD (a build may have been an mbx cache hit; rebuild before measuring)"
exit 1
