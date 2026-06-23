#!/usr/bin/env sh
# ./build.sh PROGRAM.alpha   ->   build/PROGRAM.exe   (a standalone Windows exe)
#
# Pure alpha, no Rust: beta_x64_windows.exe (which is itself the alpha seed with the
# assembler tape in its hole) assembles the program to bytecode; we memcpy that into a
# fresh copy of the alpha seed. A built program is the seed with its tape stamped in.
set -e
cd "$(dirname "$0")"
mkdir -p build
SEED=../alpha/alpha_x64_windows.exe

SRC=${1:-examples/multiply.alpha}
NAME=$(basename "$SRC" .alpha)
OUT="build/$NAME.exe"

# 1. .alpha text -> bytecode, via beta (the assembler, running on the seed)
./beta_x64_windows.exe < "$SRC" > "build/$NAME.tape"
TLEN=$(wc -c < "build/$NAME.tape")
[ $((TLEN + 4)) -le 32768 ] || { echo "FAIL: $NAME tape is $TLEN B, exceeds the seed's 32 KB hole" >&2; exit 1; }

# 2. copy the seed and memcpy [4-byte LE length][bytecode] into its hole (file offset 0x1400)
cp "$SEED" "$OUT"
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((TLEN & 255)) $(((TLEN >> 8) & 255)) $(((TLEN >> 16) & 255)) $(((TLEN >> 24) & 255)))" \
    | dd of="$OUT" bs=1 seek=5120 conv=notrunc status=none
dd if="build/$NAME.tape" of="$OUT" bs=1 seek=5124 conv=notrunc status=none

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
