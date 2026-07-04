#!/usr/bin/env sh
# ∀-INPUT THEOREM — the summit's per-vector input proofs made UNIVERSAL.
#
# input-tv.sh proves count() on particular input vectors. count-forall.elab proves it for ALL inputs at
# once — ∀xs ∀n. count(xs, n) = len(xs) + n — by structural induction on xs, the induction hypothesis
# instantiated at the SHIFTED accumulator (the shape recx was added to the kernel to express). This gate
# elaborates the named-binder proof and requires BOTH trusted checkers (check.beta AND check_ref.py) to
# accept it, and a PERTURBED goal (off by one) to be rejected by both — so acceptance is meaningful.
# The checker.gamma THIRD leg runs the proof through refcert_to_gamma (extended to the full proof
# grammar) on interp.beta — so all three independent checkers agree, with the perturbation rejected.
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
gverdict() {  # translate a cert to checker.gamma and run it; echoes accept/reject/undecided
  gg=$(python3 ../gamma/refcert_to_gamma.py < "$1" 2>/dev/null) || { echo untranslatable; return; }
  printf '%s\n%s\n' "$DEFS" "$gg" | "$T/interp.exe" >/dev/null 2>&1
  r=$?; [ "$r" = 1 ] && echo accept || { [ "$r" = 0 ] && echo reject || echo undecided; }
}

fail=0
python3 elab.py < count-forall.elab > "$T/good.cert" 2>/dev/null || { echo "  FAIL: elaboration errored"; exit 1; }
gb=$(cat "$T/good.cert" | "$T/check.exe")
gr=$(cat "$T/good.cert" | python3 check_ref.py)
gg=$(gverdict "$T/good.cert")
[ "$gb" = accept ] && [ "$gr" = accept ] && [ "$gg" = accept ] \
  && echo "  ok   theorem accepted by ALL THREE (check.beta=$gb check_ref=$gr checker.gamma=$gg): forall xs n. count(xs,n) = len(xs)+n" \
  || { echo "  FAIL: theorem not accepted by all three (check.beta=$gb check_ref=$gr checker.gamma=$gg)"; fail=1; }

# PERTURB: shift the goal's RHS by one successor (len(xs)+n -> s(len(xs)+n)); the proof must no longer fit.
sed 's/(all n (= (f 91 xs n) (f 21 (f 93 xs) n)))$/(all n (= (f 91 xs n) (k 3 (f 21 (f 93 xs) n))))/' \
    count-forall.elab | python3 elab.py > "$T/bad.cert" 2>/dev/null
bb=$(cat "$T/bad.cert" | "$T/check.exe")
br=$(cat "$T/bad.cert" | python3 check_ref.py)
bg=$(gverdict "$T/bad.cert")
[ "$bb" = reject ] && [ "$br" = reject ] && [ "$bg" = reject ] \
  && echo "  ok   perturbed goal (off by one) rejected by all three checkers" \
  || { echo "  FAIL: perturbed goal not rejected by all three (check.beta=$bb check_ref=$br checker.gamma=$bg)"; fail=1; }

echo "forall-input theorem (per-vector input proofs made universal; check.beta, check_ref AND checker.gamma agree, perturbation rejected): $( [ $fail = 0 ] && echo PASS || echo FAIL )"
[ $fail = 0 ]
