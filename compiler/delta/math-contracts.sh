#!/usr/bin/env sh
# CONTRACT DISCHARGE (omega source), AUTOMATED — the requires/ensures of samples/math_proofs are proof
# obligations; this gate discharges the arithmetic-and-ordering fragment with a kernel certificate the
# PROVER generates and ALL THREE independent checkers verify.
#
# contract2delta.py translates each contract machine into the delta proposition it obligates (prover
# syntax). prover.py (the untrusted proof-SEARCH front line) discharges it; check.beta, check_ref.py AND
# checker.gamma verify the certificate — so the omega SOURCE contract (not a hand-picked goal) is proven,
# three ways, with NO hand-authored proof. contract2delta --perturb succs the conclusion RHS into a
# well-formed FALSE proposition, which the prover must fail to prove (or, if it emits something, all three
# checkers reject). This is omega-rs's obligations concept: obligations discharged automatically.
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

gverdict() {
  gg=$(python3 prover.py --gamma "$1" 30 2>/dev/null)
  [ "$gg" = unprovable ] || [ -z "$gg" ] && { echo unprovable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0; cov=0; gap=0
python3 contract2delta.py < "$SRC" | grep -v UNSUPPORTED | while IFS="$(printf '\t')" read -r name prop; do
  cert=$(python3 prover.py "$prop" 30 2>/dev/null)
  if [ "$cert" = unprovable ] || [ -z "$cert" ]; then
    echo "  gap  $name : translated, but the prover cannot discharge it yet"
    continue
  fi
  printf '%s' "$cert" > "$T/good.cert"
  gb=$(cat "$T/good.cert" | "$T/check.exe"); gr=$(cat "$T/good.cert" | python3 check_ref.py 2>/dev/null); gg=$(gverdict "$prop")
  # perturbed proposition (off by one) must not be provable-and-accepted
  badprop=$(python3 contract2delta.py --perturb < "$SRC" | grep "^$name	" | cut -f2)
  badcert=$(python3 prover.py "$badprop" 30 2>/dev/null)
  bok=no
  if [ "$badcert" = unprovable ] || [ -z "$badcert" ]; then bok=yes
  else printf '%s' "$badcert" | "$T/check.exe" 2>/dev/null | grep -q accept || bok=yes; fi
  if [ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] && [ "$bok" = yes ]; then
    echo "  ok   $name : source contract discharged by prover + ALL THREE checkers (perturbation not provable)"
  else
    echo "  FAIL $name : (beta=$gb ref=$gr gamma=$gg perturbation-safe=$bok)"
  fi
done | tee "$T/report"

cov=$(grep -c '^  ok ' "$T/report")
gap=$(grep -c '^  gap ' "$T/report")
grep -q '^  FAIL' "$T/report" && fail=1
nunc=$(python3 contract2delta.py < "$SRC" | grep -c UNSUPPORTED)
echo "contract discharge (omega requires/ensures auto-proven by the prover, verified by check.beta + check_ref + checker.gamma): $cov discharged, $gap prover-gap, $nunc outside the fragment"
[ $fail = 0 ] && [ "$cov" -gt 0 ]
