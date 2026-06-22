#!/usr/bin/env sh
# Build the Beta compiler (the structured machines/states/transitions language) and
# verify its self-hosting fixed point. Beta is one translation unit split across the
# files in src/ (no module system — they concatenate in order): front end, then
# per-arch backend, then per-format backend. Bootstrapped by beta-rs for now;
# eventually reached by climbing up from the Alpha tape VM (../alpha). Output -> build/.
set -e
cd "$(dirname "$0")"
mkdir -p build

ONRAMP=../beta-rs/target/debug/beta.exe
SOURCES="src/beta.alp src/x64.alp src/pe.alp"
(cd ../beta-rs && cargo build -q)

"$ONRAMP" $SOURCES build/beta0.exe              # Rust on-ramp builds the compiler
cat $SOURCES | ./build/beta0.exe > build/beta1.exe   # the Beta-built compiler builds itself
cat $SOURCES | ./build/beta1.exe > build/beta2.exe   # ...and again

if cmp -s build/beta1.exe build/beta2.exe; then
    echo "self-hosting fixed point holds: beta1 == beta2 ($(wc -c < build/beta1.exe) bytes)"
else
    echo "FIXED POINT BROKEN: beta1 != beta2" >&2
    exit 1
fi
