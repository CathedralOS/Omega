#!/usr/bin/env sh
# LOGIC-DIAMOND FUZZER -- broad random coverage of the two/three trust-anchor checkers agreeing on
# PROPOSITIONAL LOGIC proofs (the ->/&/+/bot intro+elim rules). seam-fuzz fuzzes the reducer and
# checker-diamond-fuzz the equality conversion; this fuzzes the logical typing rules, cross-checked across
# all checkers only at the ~25 curated checker-diamond.sh cases otherwise. Generates hundreds of valid
# tautology-schema proofs at random atoms and requires every checker to ACCEPT each against its goal and
# REJECT it against a perturbed goal. A disagreement is a bug in one checker's logic. Needs python3.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "logic-diamond fuzz: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# Third oracle: the type-checked checker, mechanically type-erased to what the interpreter runs.
TYPED=""
if python3 ../gamma/erase_types.py < ../gamma/checker_typed.gamma > "$T/erased.gamma" 2>/dev/null; then
  TYPED="$T/erased.gamma"
fi
python3 logic-diamond-fuzz.py "$T/check.exe" "$T/interp.exe" ../gamma/checker.gamma "$TYPED" "${1:-60}"
