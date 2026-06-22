#!/usr/bin/env sh
# Reproduce the Alpha bootstrap from the committed seed — NO Rust, NO Python.
#
# One hand-audited tool (seed/hex0.exe) materializes everything from committed flat-hex
# inputs; then the assembler reproduces its own tape (a fixed point), which self-verifies
# the whole chain. To regenerate the committed inputs after editing a source listing,
# use dev-regen.sh (that is the only place Rust/Python appear). Outputs -> build/.
set -e
cd "$(dirname "$0")"
mkdir -p build
HEX0=seed/hex0.exe

# 1. the seed reproduces itself from its own flat hex
"$HEX0" < seed/hex0.flat.hex > build/hex0.exe
cmp -s seed/hex0.exe build/hex0.exe || { echo "FAIL: hex0 does not self-reproduce" >&2; exit 1; }

# 2. hex0 materializes the tape VM and the assembler tape from committed flat hex
"$HEX0" < seed/vm.flat.hex             > build/vm.exe
"$HEX0" < seed/assembler.tape.flat.hex > build/assembler.tape

# 3. the assembler reproduces its own tape (fixed point) -> the bootstrap self-verifies
#    (stdin = 4-byte LE length prefix, the assembler tape, then the assembler's source)
LEN=$(wc -c < build/assembler.tape)
prefix() { printf '%02x %02x %02x %02x' \
    $((LEN & 255)) $(((LEN >> 8) & 255)) $(((LEN >> 16) & 255)) $(((LEN >> 24) & 255)) | "$HEX0"; }
{ prefix; cat build/assembler.tape; cat seed/assembler.num; } | ./build/vm.exe > build/out.tape

if cmp -s build/assembler.tape build/out.tape; then
    echo "OK: hex0 -> vm + assembler; assembler self-hosts. No Rust, no Python."
else
    echo "FAIL: assembler self-host" >&2; exit 1
fi
