#!/usr/bin/env sh
# ./build.sh PROG.beta  ->  build/PROG.exe   (a standalone seed exe)
#
# The chain: beta-lang compiles .beta -> Alpha assembly; the assembler (../beta)
# lowers assembly -> a tape; the tape is stamped into the alpha seed. beta-lang
# is the throwaway Rust on-ramp for the Beta compiler. Per-platform seed +
# stamping live in ../alpha/seed_env.sh.
set -e
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
mkdir -p build
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED

SRC=${1:-examples/answer.beta}
NAME=$(basename "$SRC" .beta)

cargo run --quiet < "$SRC"            > "build/$NAME.asm"    # .beta -> Alpha assembly
"$ASM"            < "build/$NAME.asm" > "build/$NAME.tape"   # assembly -> tape
L=$(wc -c < "build/$NAME.tape")
[ $((L + 4)) -le "$HOLE_SIZE" ] || { echo "FAIL: $NAME tape is $L B, exceeds the seed's tape hole ($HOLE_SIZE B)" >&2; exit 1; }

stamp_seed "build/$NAME.tape" "$SEED" "build/$NAME.exe"
echo "built build/$NAME.exe  (beta -> asm -> tape -> stamp)"
