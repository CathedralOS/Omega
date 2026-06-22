#!/usr/bin/env sh
# ./build.sh PROGRAM.alp   ->   build/PROGRAM.exe   (a standalone Windows exe)
#
# Beta is the assembler, written in alpha. A built program is just the alpha seed
# binary (../alpha/alpha_x64_windows.exe) with the program's bytes memcpy'd into its
# hole. One hand-audited binary; every program is that binary with its code stamped in.
set -e
cd "$(dirname "$0")"
mkdir -p build
SEED=../alpha/alpha_x64_windows.exe

SRC=${1:-examples/multiply.alp}
NAME=$(basename "$SRC" .alp)
OUT="build/$NAME.exe"

# 1. .alp text -> bytecode  (Rust on-ramp; goes away once beta assembles its own source)
(cd ../beta-rs && cargo build -q 2>/dev/null)
../beta-rs/target/debug/assembler.exe "$SRC" "build/$NAME.tape"
TLEN=$(wc -c < "build/$NAME.tape")

# 2. copy the seed and memcpy [4-byte LE length][bytecode] into its hole (file offset 0x1400)
cp "$SEED" "$OUT"
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((TLEN & 255)) $(((TLEN >> 8) & 255)) $(((TLEN >> 16) & 255)) $(((TLEN >> 24) & 255)))" \
    | dd of="$OUT" bs=1 seek=5120 conv=notrunc status=none
dd if="build/$NAME.tape" of="$OUT" bs=1 seek=5124 conv=notrunc status=none

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the seed's hole)"
