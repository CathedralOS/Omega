#!/usr/bin/env sh
# Build the Alpha assembler and verify the tape VM's self-hosting fixed point.
# The trust root is the VM alone; the assembler is just a tape it runs.
set -e
cd "$(dirname "$0")"

ASSEMBLER=../alpha-rs/target/debug/assembler.exe
VM=../alpha-rs/target/debug/vm.exe
(cd ../alpha-rs && cargo build -q)

"$ASSEMBLER" assembler.alp assembler0.tape        # Rust on-ramp assembles assembler.alp
"$ASSEMBLER" --num assembler.alp > assembler.num   # numeric form (the assembler reads numbers)
"$VM" assembler0.tape < assembler.num > assembler1.tape   # the assembler assembles ITSELF
"$VM" assembler1.tape < assembler.num > assembler2.tape   # ...and again

if cmp -s assembler1.tape assembler2.tape; then
    echo "self-hosting fixed point holds: assembler1 == assembler2 ($(wc -c < assembler1.tape) bytes)"
else
    echo "FIXED POINT BROKEN: assembler1 != assembler2" >&2
    exit 1
fi
