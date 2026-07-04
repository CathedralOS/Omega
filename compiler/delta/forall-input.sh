#!/usr/bin/env sh
# ∀-INPUT THEOREM — the summit's per-vector input proofs made UNIVERSAL.
#
# input-tv.sh proves count() on particular input vectors. count-forall.elab proves it for ALL inputs at
# once — ∀xs ∀n. count(xs, n) = len(xs) + n — by structural induction on xs, the induction hypothesis
# instantiated at the SHIFTED accumulator (the shape recx was added to the kernel to express). This gate
# elaborates the named-binder proof and requires BOTH trusted checkers (check.beta AND check_ref.py) to
# accept it, and a PERTURBED goal (off by one) to be rejected by both — so acceptance is meaningful.
# (The checker.gamma third leg awaits a full induction-proof -> gamma translator + def/use support; the
# meaning-cert refl translator does not cover rec/eqelim/lemmas. Tracked as ∀-input climb step 4b.)
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "forall-input: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "forall-input FAIL — bc build"; exit 1; }
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
../beta-lang-rs/build/bc.exe < check.beta > "$T/c.asm" 2>/dev/null \
  && "$ASM" < "$T/c.asm" > "$T/c.tape" 2>/dev/null \
  && stamp_seed "$T/c.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 \
  || { echo "forall-input FAIL — build check.beta"; exit 1; }

fail=0
python3 elab.py < count-forall.elab > "$T/good.cert" 2>/dev/null || { echo "  FAIL: elaboration errored"; exit 1; }
gb=$(cat "$T/good.cert" | "$T/check.exe")
gr=$(cat "$T/good.cert" | python3 check_ref.py)
[ "$gb" = accept ] && [ "$gr" = accept ] \
  && echo "  ok   theorem accepted (check.beta=$gb check_ref=$gr): forall xs n. count(xs,n) = len(xs)+n" \
  || { echo "  FAIL: theorem not accepted by both (check.beta=$gb check_ref=$gr)"; fail=1; }

# PERTURB: shift the goal's RHS by one successor (len(xs)+n -> s(len(xs)+n)); the proof must no longer fit.
sed 's/(all n (= (f 91 xs n) (f 21 (f 93 xs) n)))$/(all n (= (f 91 xs n) (k 3 (f 21 (f 93 xs) n))))/' \
    count-forall.elab | python3 elab.py > "$T/bad.cert" 2>/dev/null
bb=$(cat "$T/bad.cert" | "$T/check.exe")
br=$(cat "$T/bad.cert" | python3 check_ref.py)
[ "$bb" = reject ] && [ "$br" = reject ] \
  && echo "  ok   perturbed goal (off by one) rejected by both checkers" \
  || { echo "  FAIL: perturbed goal not rejected by both (check.beta=$bb check_ref=$br)"; fail=1; }

echo "forall-input theorem (per-vector input proofs made universal; check.beta AND check_ref agree, perturbation rejected): $( [ $fail = 0 ] && echo PASS || echo FAIL )"
[ $fail = 0 ]
