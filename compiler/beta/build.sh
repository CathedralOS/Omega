#!/usr/bin/env sh
# ./build.sh PROGRAM.alpha   ->   build/PROGRAM.exe   (a standalone seed exe)
#
# Pure alpha, no Rust: the assembler (the alpha seed with the assembler tape in
# its hole) assembles the program to bytecode; we memcpy that into a fresh copy
# of the alpha seed. A built program IS the seed with its tape stamped in. The
# per-platform seed + stamping live in ../alpha/seed_env.sh.
set -e
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
mkdir -p build
SEED=../alpha/$ALPHA_SEED

SRC=${1:-examples/multiply.alpha}
NAME=$(basename "$SRC" .alpha)
OUT="build/$NAME.exe"

# 1. .alpha text -> bytecode, via the assembler running on the seed
"./$BETA_SEED" < "$SRC" > "build/$NAME.tape"
TLEN=$(wc -c < "build/$NAME.tape")
[ $((TLEN + 4)) -le 32768 ] || { echo "FAIL: $NAME tape is $TLEN B, exceeds the seed's 32 KB hole" >&2; exit 1; }

# 2. stamp [4-byte LE length][bytecode] into a fresh copy of the seed
stamp_seed "build/$NAME.tape" "$SEED" "$OUT"

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
