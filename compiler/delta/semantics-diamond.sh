#!/usr/bin/env sh
# SEMANTICS DIAMOND — the gamma/delta soundness seam, as a cross-check.
#
# delta.md's honest-edge #1: a proof must connect to "what a program actually does
# per the reference interpreter" — a soundness theorem at the gamma/delta seam. We
# cannot prove that theorem here, but we can EXHIBIT the seam: the checker's
# DEFINITIONAL equality (eq.beta: reduce both sides to normal form) must agree with
# the interpreter's OPERATIONAL evaluation (interp.beta running gamma's own `plus`)
# for every Peano equation. Two different routes to "is e1 = e2" — definitional vs
# operational — agreeing is evidence the checker's equality is sound w.r.t. the
# reference interpreter (not a proof; the theorem is the open problem).
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b eq.beta "$T/eq.exe"           || { echo "build eq.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# gamma defs: plus (operational addition) + neq (structural Nat equality -> 1/0)
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def neq (a b) (match a (Ze (match b (Ze 1) (w 0))) ((S x) 0) ((Su x) (match b ((Su y) (neq x y)) (w 0)))))'

PASS=0; FAIL=0
# dia DESC  EQ_BETA_INPUT(p/s/z)  GAMMA_EXPR(Su/Ze + plus)  EXPECT(equal|differ)
dia() {
  veq=$(printf '%s' "$2" | "$T/eq.exe")                              # eq.beta: equal/differ
  printf '%s\n(neq %s)\n' "$DEFS" "$3" | "$T/interp.exe" >/dev/null; n=$?  # interp: 1/0
  vop=differ; [ "$n" = 1 ] && vop=equal
  if [ "$veq" = "$vop" ] && [ "$veq" = "$4" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : definitional=$veq operational=$vop expect=$4"; fi
}
#    desc        eq.beta (definitional)                                gamma (operational, plus)                                          expect
dia "2+2 = 4"    "(p (s (s z)) (s (s z)))  (s (s (s (s z))))"          "(plus (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su Ze))))"           equal
dia "0+3 = 3"    "(p z (s (s (s z))))  (s (s (s z)))"                  "(plus Ze (Su (Su (Su Ze)))) (Su (Su (Su Ze)))"                     equal
dia "3+0 = 3"    "(p (s (s (s z))) z)  (s (s (s z)))"                  "(plus (Su (Su (Su Ze))) Ze) (Su (Su (Su Ze)))"                     equal
dia "1+2 = 3"    "(p (s z) (s (s z)))  (s (s (s z)))"                  "(plus (Su Ze) (Su (Su Ze))) (Su (Su (Su Ze)))"                     equal
dia "2+2 != 5"   "(p (s (s z)) (s (s z)))  (s (s (s (s (s z)))))"      "(plus (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su (Su Ze)))))"      differ
dia "1+1 != 1"   "(p (s z) (s z))  (s z)"                              "(plus (Su Ze) (Su Ze)) (Su Ze)"                                   differ
echo "semantics diamond (definitional eq vs operational eval): $PASS agree, $FAIL disagree"
[ "$FAIL" = 0 ] || exit 1
