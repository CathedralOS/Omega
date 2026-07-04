#!/usr/bin/env sh
# CONTRACT DISCHARGE (omega source) — the requires/ensures of samples/math_proofs are proof obligations;
# this gate discharges the arithmetic-equality subset with a kernel certificate, checked by ALL THREE
# independent checkers.
#
# contract2delta.py translates each contract machine into the delta proposition it obligates. For each
# covered contract, the gate assembles < the TRANSLATED proposition > + < a self-contained proof from
# math-contract-proofs/ > and requires check.beta, check_ref.py AND checker.gamma to accept — so the
# omega SOURCE contract (not a hand-picked goal) is discharged, three ways. contract2delta --perturb
# succs the conclusion RHS into a well-formed FALSE proposition, which all three must reject.
# The proofs are over BUILT-IN nats (z/s, + = Pl, * = Mu), so no data/fun prelude is needed and the
# proof terms translate to checker.gamma directly (natind + eqelim + have/app/lam).
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "math-contracts: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "math-contracts FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta        "$T/check.exe"  || { echo "math-contracts FAIL — build check.beta"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "math-contracts FAIL — build interp.beta"; exit 1; }
DEFS=$(cat ../gamma/checker.gamma)
SRC=../../samples/math_proofs/main.omg

gverdict() {  # translate a cert to checker.gamma, run it; echoes accept/reject/undecided
  gg=$(python3 ../gamma/refcert_to_gamma.py < "$1" 2>/dev/null) || { echo untranslatable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0; cov=0
for name in pythagorean_three_four_five add_one_congruence add_commutative multiply_distributes; do
  pf="math-contract-proofs/$name.pf"
  [ -f "$pf" ] || { echo "  FAIL $name : missing proof"; fail=1; continue; }
  prop=$(python3 contract2delta.py < "$SRC" | grep "^$name	" | cut -f2)
  [ -n "$prop" ] || { echo "  FAIL $name : contract2delta did not translate it"; fail=1; continue; }
  # SOURCE -> PROOF: the translated proposition IS the goal; the proof must discharge it
  { printf '%s\n' "$prop"; cat "$pf"; } | python3 elab.py > "$T/good.cert" 2>/dev/null \
    || { echo "  FAIL $name : elaboration errored"; fail=1; continue; }
  gb=$(cat "$T/good.cert" | "$T/check.exe"); gr=$(cat "$T/good.cert" | python3 check_ref.py); gg=$(gverdict "$T/good.cert")
  # perturbed proposition (off by one) with the SAME proof -> all three must reject
  badprop=$(python3 contract2delta.py --perturb < "$SRC" | grep "^$name	" | cut -f2)
  { printf '%s\n' "$badprop"; cat "$pf"; } | python3 elab.py > "$T/bad.cert" 2>/dev/null
  pb=$(cat "$T/bad.cert" | "$T/check.exe" 2>/dev/null); pr=$(cat "$T/bad.cert" | python3 check_ref.py 2>/dev/null); pg=$(gverdict "$T/bad.cert")
  if [ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] \
     && [ "$pb" != accept ] && [ "$pr" != accept ] && [ "$pg" != accept ]; then
    cov=$((cov+1)); echo "  ok   $name : source contract discharged by ALL THREE (perturbation rejected)"
  else
    echo "  FAIL $name : (good beta=$gb ref=$gr gamma=$gg | bad beta=$pb ref=$pr gamma=$pg)"; fail=1
  fi
done

nunc=$(python3 contract2delta.py < "$SRC" | grep -c UNSUPPORTED)
echo "contract discharge (omega requires/ensures proven by check.beta + check_ref + checker.gamma): $cov equality contracts discharged, $nunc outside the fragment (ranges / < / Bag — future)"
[ $fail = 0 ] && [ $cov -gt 0 ]
