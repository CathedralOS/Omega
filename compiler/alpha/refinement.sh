#!/usr/bin/env sh
# INSTRUCTION-LEVEL REFINEMENT — certify that Alpha machine code computes the intended function of its inputs,
# proven WITHOUT running it. The lattice already certifies meaning at the SOURCE level (gamma) and RESULTS via
# translation validation; this reaches the bottom rung: the actual bytecode the machine executes.
#
# For each hand-built loop-free arithmetic program, alpha_refinement_check.py (1) symbolically executes the
# tape to a closed-form Peano expression over its inputs, (2) differentially pins that expression to the
# concrete VM (alpha_ref.py) on random inputs, and (3) proves it equals the claimed source meaning for ALL
# inputs — handing the universal goal to the untrusted prover.py and validating its certificate with the trust
# anchor (check.beta). A correct compilation yields a proof-carrying REFINES; a wrong one yields no accepted
# proof. This is the seed of the Cathedral endgame (rungs/*.md): output certifies the compiler, down to the
# instructions. The symbolic executor is UNTRUSTED and checked; nothing here runs in the trusted lineage.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "refinement: skipped (python3 absent)"; exit 0; }
. seed_env.sh
SEED=$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "refinement: bc build failed"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < ../delta/check.beta > "$T/c.asm" 2>/dev/null && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "refinement: check.beta build failed"; exit 1; }

echo "instruction-level refinement (alpha machine code provably computes its source meaning, checked without running it):"
python3 alpha_refinement_check.py "$T/check.exe" "$(pwd)/../beta-lang-rs/build/bc.exe" "$(pwd)/$ASM"
