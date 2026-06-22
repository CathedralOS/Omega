#!/usr/bin/env sh
# Build the Alpha compiler and verify the self-hosting fixed point.
#
# Alpha is one translation unit split across files (no module system — they are
# concatenated in this order): front end, then per-arch backend, then per-format
# backend. Adding a target (aarch64.alp) or format (elf.alp) means adding a file
# here, not editing a monolith.
set -e
cd "$(dirname "$0")"

ONRAMP=../alpha-rs/target/debug/alpha.exe
SOURCES="alpha.alp x64.alp pe.alp"

# stage 0: the throwaway Rust on-ramp builds the compiler (multi-file argv)
"$ONRAMP" $SOURCES alpha0.exe

# stage 1 + 2: the Alpha-built compiler builds itself (concatenated source on stdin)
cat $SOURCES | ./alpha0.exe > alpha1.exe
cat $SOURCES | ./alpha1.exe > alpha2.exe

if cmp -s alpha1.exe alpha2.exe; then
    echo "self-hosting fixed point holds: alpha1 == alpha2 ($(wc -c < alpha1.exe) bytes)"
else
    echo "FIXED POINT BROKEN: alpha1 != alpha2" >&2
    exit 1
fi
