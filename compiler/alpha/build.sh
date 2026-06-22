#!/usr/bin/env sh
# Build the Alpha tape VM + assembler and verify both self-hosting facts:
#   1. the assembler reproduces itself  (on the Rust reference VM)
#   2. the HAND-ASSEMBLED seed VM reproduces the assembler tape (no Rust in the VM path)
# All generated files go under build/. The numeric form of the source is piped, never
# written, so there is no .num artifact.
set -e
cd "$(dirname "$0")"
mkdir -p build

ASSEMBLER=../alpha-rs/target/debug/assembler.exe
VM=../alpha-rs/target/debug/vm.exe
(cd ../alpha-rs && cargo build -q)

# the Rust on-ramp mints the initial assembler tape from its mnemonic source
"$ASSEMBLER" src/assembler.alp build/assembler.tape

# 1. self-host on the Rust reference VM: the assembler assembles its own (numeric)
#    source twice and reaches a byte-identical fixed point
"$ASSEMBLER" --num src/assembler.alp | "$VM" build/assembler.tape > build/gen1.tape
"$ASSEMBLER" --num src/assembler.alp | "$VM" build/gen1.tape      > build/gen2.tape
if cmp -s build/assembler.tape build/gen1.tape && cmp -s build/gen1.tape build/gen2.tape; then
    echo "self-host (Rust VM): assembler.tape == gen1 == gen2 ($(wc -c < build/assembler.tape) bytes)"
else
    echo "FAIL: Rust-VM self-host" >&2; exit 1
fi

# 2. grounding: the hand-assembled seed VM (materialized from seed/vm.hex — no Rust,
#    no LLVM) reproduces the assembler tape. The VM reads the tape from a 4-byte LE
#    stdin length prefix, then the program's input (the numeric source) follows.
python seed/materialize.py seed/vm.hex build/vm.exe
LEN=$(wc -c < build/assembler.tape)
prefix() { python -c "import sys,struct; sys.stdout.buffer.write(struct.pack('<I',$LEN))"; }
{ prefix; cat build/assembler.tape; "$ASSEMBLER" --num src/assembler.alp; } | ./build/vm.exe > build/hand.tape
if cmp -s build/assembler.tape build/hand.tape; then
    echo "grounding (hand seed VM): reproduces assembler.tape, no Rust in the VM path"
else
    echo "FAIL: hand-VM grounding" >&2; exit 1
fi
