#!/usr/bin/env sh
# ∀-INPUT THEOREM — the summit's per-vector input proofs made UNIVERSAL, and MECHANICAL.
#
# count/k-family: fold gains a constant per element. sum-forall: fold gains the ELEMENT (needs uadd
# proven a commutative monoid). input-tv.sh proves an input loop's meaning on particular input vectors. This gate proves it for ALL
# inputs at once — ∀xs ∀n. fold(xs, n) = agg(xs) + n — by structural induction on xs, the induction
# hypothesis instantiated at the SHIFTED accumulator (the shape recx was added to the kernel to express).
#
# count-forall.elab is the hand-authored k=1 (count/len) reference; forall-gen.py MECHANICALLY emits the
# whole constant-increment family (fold that adds k successors per element -> k*len), proving the proof
# was a reusable SCHEMA. Every theorem must be accepted by ALL THREE independent checkers (check.beta,
# check_ref.py, checker.gamma via refcert_to_gamma on interp.beta), and an off-by-one perturbed goal must
# be rejected by all three — so acceptance is meaningful.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "forall-input: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "forall-input FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" 2>/dev/null && "$ASM" < "$T/x.asm" > "$T/x.tape" 2>/dev/null && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta        "$T/check.exe"  || { echo "forall-input FAIL — build check.beta"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "forall-input FAIL — build interp.beta"; exit 1; }
DEFS=$(cat ../gamma/checker.gamma)

gverdict() {  # translate a cert to checker.gamma, run it; echoes accept/reject/undecided
  gg=$(python3 ../gamma/refcert_to_gamma.py < "$1" 2>/dev/null) || { echo untranslatable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0
# verify one elab SOURCE (on stdin via $1 command): theorem accepted by all three, perturbation rejected.
verify() {  # $1 = label, $2 = shell command emitting the .elab source
  eval "$2" > "$T/src.elab"
  python3 elab.py < "$T/src.elab" > "$T/good.cert" 2>/dev/null || { echo "  FAIL $1: elaboration errored"; fail=1; return; }
  ab=$(cat "$T/good.cert" | "$T/check.exe"); ar=$(cat "$T/good.cert" | python3 check_ref.py); ag=$(gverdict "$T/good.cert")
  [ "$ab" = accept ] && [ "$ar" = accept ] && [ "$ag" = accept ] \
    && echo "  ok   $1 accepted by all three (check.beta/check_ref/checker.gamma)" \
    || { echo "  FAIL $1: not accepted by all three (beta=$ab ref=$ar gamma=$ag)"; fail=1; }
  # PERTURB: wrap the goal RHS in one extra successor; the proof must no longer fit.
  sed '/^(all xs/ s/(f 21 (f 9\([0-9]\) xs) n)/(k 3 (f 21 (f 9\1 xs) n))/' "$T/src.elab" \
    | python3 elab.py > "$T/bad.cert" 2>/dev/null
  pb=$(cat "$T/bad.cert" | "$T/check.exe"); pr=$(cat "$T/bad.cert" | python3 check_ref.py); pg=$(gverdict "$T/bad.cert")
  [ "$pb" = reject ] && [ "$pr" = reject ] && [ "$pg" = reject ] \
    && echo "  ok   $1 perturbation (off by one) rejected by all three" \
    || { echo "  FAIL $1: perturbation not rejected by all three (beta=$pb ref=$pr gamma=$pg)"; fail=1; }
}

verify "count-forall (hand-authored k=1)" "cat count-forall.elab"
verify "generated k=1 (count -> len)"     "python3 forall-gen.py 1"
verify "generated k=2 (2 per elem -> 2*len)" "python3 forall-gen.py 2"
verify "generated k=3 (3 per elem -> 3*len)" "python3 forall-gen.py 3"
verify "sum-forall (add-the-element fold; uadd commutative monoid)" "cat sum-forall.elab"

echo "forall-input theorem (per-vector input proofs made universal AND mechanical; all three checkers agree, perturbations rejected): $( [ $fail = 0 ] && echo PASS || echo FAIL )"
[ $fail = 0 ]
