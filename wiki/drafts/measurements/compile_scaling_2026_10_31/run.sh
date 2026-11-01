#!/bin/bash
# Interleaved cold/warm compile-only measurements with --timings.
S="${SCALING_OUT:?set SCALING_OUT to an output directory}"; mkdir -p "$S/m"
cd /c/SoftwareDevelopmentKits/Omega/.agents/worktree/pscale
O=./target/release/omega.exe
for round in 1 2 3; do
  for p in empty hello dungeon; do
    c=$S/cache/m-$p-$round; rm -rf $c; mkdir -p $c
    for mode in cold warm1 warm2; do
      out=$S/m/$p-$round-$mode.txt
      a=$(date +%s%N); LOCALAPPDATA=$c $O --timings build/pscale/$p/main.omg < /dev/null > $out 2>&1; e=$?; b=$(date +%s%N)
      echo "$p round=$round $mode exit=$e wall_ms=$(( (b-a)/1000000 )) $(grep 'total elapsed' $out | awk '{print "self_ms="$1}') $(grep 'Stage 05' $out | awk '{print "s05_ms="$1}')"
    done
  done
done
