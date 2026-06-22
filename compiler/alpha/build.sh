#!/usr/bin/env sh
# ./build.sh PROGRAM.alp   ->   build/PROGRAM.exe   (a standalone Windows exe)
#
# A built program is just vm.exe (the binary from god) with the program's bytes
# memcpy'd into its .tape hole. That's the whole idea: one hand-audited binary, and
# every program is that binary with its code stamped in.
set -e
cd "$(dirname "$0")"
mkdir -p build

SRC=${1:-src/multiply.alp}
NAME=$(basename "$SRC" .alp)
OUT="build/$NAME.exe"

# 1. .alp -> bytecode  (assembler on-ramp; will be replaced by the in-Alpha compiler)
(cd ../alpha-rs && cargo build -q 2>/dev/null)
../alpha-rs/target/debug/assembler.exe "$SRC" "build/$NAME.tape"
TLEN=$(wc -c < "build/$NAME.tape")

# 2. copy the god binary and memcpy [4-byte LE length][bytecode] into its hole (@0x1400)
cp vm.exe "$OUT"
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((TLEN & 255)) $(((TLEN >> 8) & 255)) $(((TLEN >> 16) & 255)) $(((TLEN >> 24) & 255)))" \
    | dd of="$OUT" bs=1 seek=5120 conv=notrunc status=none
dd if="build/$NAME.tape" of="$OUT" bs=1 seek=5124 conv=notrunc status=none

echo "built $OUT  ($(wc -c < "$OUT") bytes; $TLEN bytes of program in the hole)"
