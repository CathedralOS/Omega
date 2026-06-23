#!/usr/bin/env sh
# ./build.sh PROG.beta  ->  build/PROG.exe   (a standalone Windows exe)
#
# The chain: beta-lang compiles .beta -> Alpha assembly; the assembler (../beta)
# lowers assembly -> a tape; the tape is memcpy'd into the alpha seed. beta-lang is
# the throwaway Rust on-ramp for the Beta compiler (to be transcribed to assembly).
set -e
cd "$(dirname "$0")"
mkdir -p build
SEED=../alpha/alpha_x64_windows.exe
ASM=../beta/beta_x64_windows.exe

SRC=${1:-examples/answer.beta}
NAME=$(basename "$SRC" .beta)

cargo run --quiet < "$SRC"            > "build/$NAME.asm"    # .beta -> Alpha assembly
"$ASM"            < "build/$NAME.asm" > "build/$NAME.tape"   # assembly -> tape
L=$(wc -c < "build/$NAME.tape")
[ $((L + 4)) -le 32768 ] || { echo "FAIL: $NAME tape is $L B, exceeds the seed's 32 KB hole" >&2; exit 1; }

cp "$SEED" "build/$NAME.exe"
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of="build/$NAME.exe" bs=1 seek=5120 conv=notrunc status=none
dd if="build/$NAME.tape" of="build/$NAME.exe" bs=1 seek=5124 conv=notrunc status=none
echo "built build/$NAME.exe  (beta -> asm -> tape -> stamp)"
