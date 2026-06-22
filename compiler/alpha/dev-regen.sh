#!/usr/bin/env sh
# DEV ONLY — regenerate the committed bootstrap inputs after editing a source listing
# (seed/hex0.hex, seed/vm.hex, or src/assembler.alp). This is the ONLY place Rust and
# Python appear; the reproduce path (build.sh) needs neither. After running this,
# run ./build.sh to verify, then commit the updated seed/ files.
set -e
cd "$(dirname "$0")"
mkdir -p build
(cd ../alpha-rs && cargo build -q)
ASSEMBLER=../alpha-rs/target/debug/assembler.exe

# hand-assembled listings -> binary (materialize.py resolves labels) -> committed flat hex
python seed/materialize.py seed/hex0.hex seed/hex0.exe
xxd -p seed/hex0.exe > seed/hex0.flat.hex
python seed/materialize.py seed/vm.hex build/vm.exe
xxd -p build/vm.exe > seed/vm.flat.hex

# the assembler: .alp source -> bytecode tape (flat hex) + numeric source
"$ASSEMBLER" src/assembler.alp build/assembler.tape
xxd -p build/assembler.tape > seed/assembler.tape.flat.hex
"$ASSEMBLER" --num src/assembler.alp > seed/assembler.num

echo "regenerated seed/ artifacts; run ./build.sh to verify, then commit seed/"
