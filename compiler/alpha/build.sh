#!/usr/bin/env sh
# Build the Alpha assembler and verify the tape VM's self-hosting fixed point.
# The trust root is the VM alone; the assembler is just a tape it runs.
set -e
cd "$(dirname "$0")"

ASM=../alpha-rs/target/debug/asm.exe
VM=../alpha-rs/target/debug/vm.exe
(cd ../alpha-rs && cargo build -q)

"$ASM" as.asm as0.tape          # Rust on-ramp assembles as.asm -> as0.tape
"$ASM" --num as.asm > as.num     # numeric form of as.asm (the asm-in-asm reads numbers)
"$VM" as0.tape < as.num > as1.tape   # the assembler assembles ITSELF
"$VM" as1.tape < as.num > as2.tape   # ...and again

if cmp -s as1.tape as2.tape; then
    echo "self-hosting fixed point holds: as1 == as2 ($(wc -c < as1.tape) bytes)"
else
    echo "FIXED POINT BROKEN: as1 != as2" >&2
    exit 1
fi
