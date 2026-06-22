#!/usr/bin/env sh
# Build the Beta compiler (the structured machines/states/transitions language,
# formerly "alpha") and verify its self-hosting fixed point. Beta is one translation
# unit split across files (no module system — concatenated in this order): front end,
# then per-arch backend, then per-format backend. Beta is bootstrapped by beta-rs for
# now; eventually it is reached by climbing up from the Alpha tape VM (../alpha).
set -e
cd "$(dirname "$0")"

ONRAMP=../beta-rs/target/debug/beta.exe
SOURCES="beta.alp x64.alp pe.alp"

# stage 0: the throwaway Rust on-ramp builds the compiler (multi-file argv)
"$ONRAMP" $SOURCES beta0.exe

# stage 1 + 2: the Beta-built compiler builds itself (concatenated source on stdin)
cat $SOURCES | ./beta0.exe > beta1.exe
cat $SOURCES | ./beta1.exe > beta2.exe

if cmp -s beta1.exe beta2.exe; then
    echo "self-hosting fixed point holds: beta1 == beta2 ($(wc -c < beta1.exe) bytes)"
else
    echo "FIXED POINT BROKEN: beta1 != beta2" >&2
    exit 1
fi
