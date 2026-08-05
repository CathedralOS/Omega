#!/usr/bin/env sh
# LOGIC SOUNDNESS SEAM -- the propositional-logic pillar bridged to classical TRUTH.
#
# The fourth operational seam (after semantics-diamond=equality, induction-soundness=universals,
# predicate-soundness=inductive predicates). check.beta's logic is INTUITIONISTIC, so everything it
# proves is CLASSICALLY valid: for each propositional proof it accepts, an independent truth-table
# oracle must find the goal a TAUTOLOGY, and a perturbed genuine NON-tautology must be REJECTED. Two
# independent routes -- a kernel typing derivation and a semantic decision -- agreeing is evidence the
# checker's logic is sound (not a proof; the theorem is the open problem). Needs python3.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "logic-soundness: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe" || { echo "build check.beta failed"; exit 1; }
python3 logic-soundness.py "$T/check.exe" "${1:-100}"
