#!/usr/bin/env sh
# CONTRACT DISCHARGE (omega source) — the requires/ensures of samples/math_proofs are proof obligations;
# this gate discharges the arithmetic-equality subset with a kernel certificate.
#
# contract2delta.py translates each contract machine into the delta proposition it obligates. math-
# contracts.elab proves those propositions (built-in nat arithmetic: pythagorean by refl, congruence,
# commutativity and distributivity by induction). The gate TIES SOURCE TO PROOF: for each covered
# contract it assembles <library defs> + <the translated proposition> + (use N) and requires check.beta
# AND check_ref to accept — which they do only if the translated proposition equals the proven lemma's
# type. So the omega SOURCE contract, not a hand-picked goal, is what the kernel discharges. A perturbed
# proposition (off by one) must be rejected.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "math-contracts: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "math-contracts FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < check.beta > "$T/c.asm" 2>/dev/null \
  && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 \
  || { echo "math-contracts FAIL — build check.beta"; exit 1; }

SRC=../../samples/math_proofs/main.omg
# the proof library = math-contracts.elab up to (but not including) its top-level goal line
sed '/^; top-level goal/,$d' math-contracts.elab > "$T/lib.elab"

fail=0
# contract name -> the def N that proves it
map='pythagorean_three_four_five:2 add_one_congruence:3 add_commutative:4 multiply_distributes:6'
cov=0
for pair in $map; do
  name=${pair%:*}; defn=${pair#*:}
  prop=$(python3 contract2delta.py < "$SRC" | grep "^$name	" | cut -f2)
  [ -n "$prop" ] || { echo "  FAIL $name : contract2delta did not translate it"; fail=1; continue; }
  # SOURCE -> PROOF: the translated proposition must be exactly what (use defn) proves
  { cat "$T/lib.elab"; printf '\n%s\n(use %s)\n' "$prop" "$defn"; } | python3 elab.py > "$T/good.cert" 2>/dev/null \
    || { echo "  FAIL $name : elaboration errored"; fail=1; continue; }
  gb=$(cat "$T/good.cert" | "$T/check.exe"); gr=$(cat "$T/good.cert" | python3 check_ref.py)
  # perturbed: contract2delta --perturb succs the conclusion RHS -> a well-formed FALSE proposition
  badprop=$(python3 contract2delta.py --perturb < "$SRC" | grep "^$name	" | cut -f2)
  { cat "$T/lib.elab"; printf '\n%s\n(use %s)\n' "$badprop" "$defn"; } | python3 elab.py > "$T/bad.cert" 2>/dev/null
  pb=$(cat "$T/bad.cert" | "$T/check.exe" 2>/dev/null)
  if [ "$gb" = accept ] && [ "$gr" = accept ] && [ "$pb" != accept ]; then
    cov=$((cov+1)); echo "  ok   $name : source contract discharged (check.beta+check_ref; perturbation rejected)"
  else
    echo "  FAIL $name : not discharged (beta=$gb ref=$gr perturbed_beta=$pb)"; fail=1
  fi
done

nunc=$(python3 contract2delta.py < "$SRC" | grep -c UNSUPPORTED)
echo "contract discharge (omega requires/ensures proven by the anchor): $cov equality contracts discharged, $nunc outside the fragment (ranges / < / Bag — future)"
[ $fail = 0 ] && [ $cov -gt 0 ]
