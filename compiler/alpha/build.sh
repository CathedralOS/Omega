#!/usr/bin/env sh
# Build the Alpha tape VM + assembler and verify self-hosting — with NO Python in the
# loop. The VM is materialized from a committed flat-hex listing by the hand-assembled
# hex0 seed (seed/hex0.exe), which reproduces itself first. Only the assembler TAPE is
# still minted by the Rust on-ramp (the remaining non-hand-assembled bit). All
# generated files go under build/.
set -e
cd "$(dirname "$0")"
mkdir -p build

ASSEMBLER=../alpha-rs/target/debug/assembler.exe
(cd ../alpha-rs && cargo build -q)

# 1. seed integrity: the committed hex0 reproduces itself from its own flat hex
seed/hex0.exe < seed/hex0.flat.hex > build/hex0.exe
cmp -s seed/hex0.exe build/hex0.exe || { echo "FAIL: hex0 does not self-reproduce" >&2; exit 1; }
echo "hex0 self-reproduces (the seed is consistent)"

# 2. materialize the tape VM with hex0 — no Python, no LLVM
seed/hex0.exe < seed/vm.flat.hex > build/vm.exe
echo "materialized vm.exe via hand-assembled hex0 (no Python in the VM path)"

# 3. grounding self-host: the hand VM runs the assembler tape and reproduces it
"$ASSEMBLER" src/assembler.alp build/assembler.tape      # Rust on-ramp mints the tape
LEN=$(wc -c < build/assembler.tape)
# 4-byte little-endian length prefix, emitted as hex through our own hex0 (no Python)
prefix() {
    printf '%02x %02x %02x %02x' \
        $((LEN & 255)) $(((LEN >> 8) & 255)) $(((LEN >> 16) & 255)) $(((LEN >> 24) & 255)) | seed/hex0.exe
}
{ prefix; cat build/assembler.tape; "$ASSEMBLER" --num src/assembler.alp; } | ./build/vm.exe > build/hand.tape
if cmp -s build/assembler.tape build/hand.tape; then
    echo "grounding: hand VM reproduces assembler.tape ($(wc -c < build/hand.tape) bytes)"
else
    echo "FAIL: hand-VM grounding" >&2; exit 1
fi
