#!/usr/bin/env sh
# Verify beta self-hosts: beta assembles its own source (assembler.alp) and the result,
# stamped into the seed, is byte-identical to beta_x64_windows.exe. No Rust.
set -e
cd "$(dirname "$0")"
SEED=../alpha/alpha_x64_windows.exe
mkdir -p build

./beta_x64_windows.exe < assembler.alp > build/assembler.tape
L=$(wc -c < build/assembler.tape)
[ $((L + 4)) -le 32768 ] || { echo "FAIL: assembler tape is $L B, exceeds the seed's 32 KB hole" >&2; exit 1; }
cp "$SEED" build/beta.exe
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of=build/beta.exe bs=1 seek=5120 conv=notrunc status=none
dd if=build/assembler.tape of=build/beta.exe bs=1 seek=5124 conv=notrunc status=none

if cmp -s beta_x64_windows.exe build/beta.exe; then
    echo "self-host ✓ — beta rebuilds itself from assembler.alp, byte-identical, no Rust"
else
    echo "FAIL: beta does not reproduce itself" >&2; exit 1
fi
