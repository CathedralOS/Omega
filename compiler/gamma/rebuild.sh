#!/usr/bin/env sh
# Rebuild gamma_x64_windows.exe = the alpha seed with gamma's tape stamped into its hole.
# gamma.alp is assembled by beta (the rung below). No Rust, no Python.
set -e
cd "$(dirname "$0")"
SEED=../alpha/alpha_x64_windows.exe
mkdir -p build
../beta/beta_x64_windows.exe < gamma.alp > build/gamma.tape
L=$(wc -c < build/gamma.tape)
[ $((L + 4)) -le 8192 ] || { echo "FAIL: gamma tape is $L B, exceeds the seed's 8 KB hole" >&2; exit 1; }
cp "$SEED" gamma_x64_windows.exe
printf "$(printf '\\%03o\\%03o\\%03o\\%03o' $((L & 255)) $(((L >> 8) & 255)) $(((L >> 16) & 255)) $(((L >> 24) & 255)))" \
    | dd of=gamma_x64_windows.exe bs=1 seek=5120 conv=notrunc status=none
dd if=build/gamma.tape of=gamma_x64_windows.exe bs=1 seek=5124 conv=notrunc status=none
echo "rebuilt gamma_x64_windows.exe ($L-byte tape)"
