#!/usr/bin/env sh
# Ground the Alpha assembler on the HAND-ASSEMBLED VM — no Rust/LLVM in the VM path.
#
# The VM (vm.exe) is materialized from the hand-authored machine-code listing vm.hex.
# It loads a tape from a 4-byte little-endian length prefix on stdin, then runs it;
# the rest of stdin is the program's input. We feed it the assembler tape + the
# assembler's own (numeric) source and confirm it reproduces the assembler tape.
set -e
cd "$(dirname "$0")"

ASM=../../alpha-rs/target/debug/asm.exe       # Rust on-ramp: only to make the *initial* tape
python build.py vm.hex vm.exe                 # the hand-assembled trust-root VM

"$ASM" ../as.asm as.tape                       # initial assembler tape (will be reproduced)
"$ASM" --num ../as.asm > as.num                 # the assembler's source in numeric form
LEN=$(wc -c < as.tape)
prefix() { python -c "import sys,struct; sys.stdout.buffer.write(struct.pack('<I',$LEN))"; }

# gen 1: the HAND VM runs as.tape, assembling as.num -> as1.tape
( prefix; cat as.tape; cat as.num ) | ./vm.exe > as1.tape
# gen 2: the HAND VM runs the tape IT just produced, again -> as2.tape
( prefix; cat as1.tape; cat as.num ) | ./vm.exe > as2.tape

if cmp -s as.tape as1.tape && cmp -s as1.tape as2.tape; then
    echo "hand-VM self-host holds: as.tape == as1 == as2 ($(wc -c < as1.tape) bytes), no Rust in the VM path"
else
    echo "SELF-HOST BROKEN" >&2; exit 1
fi
rm -f vm.exe as.tape as.num as1.tape as2.tape
