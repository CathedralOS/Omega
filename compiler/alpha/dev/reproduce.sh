#!/usr/bin/env sh
# Reproduce the alpha seed binary from its hand-written listing — no Python.
# hex0 (itself hand-assembled) turns the committed flat hex into the binary; we check
# it matches the committed seed. (To audit from scratch instead: disassemble
# ../alpha_x64_windows.exe and read it against ../alpha_x64_windows.hex.)
set -e
cd "$(dirname "$0")"
./hex0.exe < alpha_x64_windows.flat.hex > alpha-seed-check.exe
if cmp -s ../alpha_x64_windows.exe alpha-seed-check.exe; then
    echo "alpha seed reproduces from source (no Python) ✓"
else
    echo "MISMATCH: ../alpha_x64_windows.exe != hex0(flat)" >&2; exit 1
fi
rm -f alpha-seed-check.exe
