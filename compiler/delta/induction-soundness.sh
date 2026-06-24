#!/usr/bin/env sh
# INDUCTION SOUNDNESS SEAM — the checker's universals vs the reference interpreter.
#
# natind is the most powerful and most soundness-critical rule: it concludes a
# statement about ALL naturals from two finite premises. semantics-diamond.sh checks
# the conversion rule (definitional `=`) against operational evaluation; this does the
# analogous thing one level up, for INDUCTIVELY proved universals. For each theorem
# `forall n. P(n)` that check.beta accepts by induction, the gamma reference
# interpreter must independently confirm `P(k)` at concrete k = 0,1,2,3,4,5. If
# induction ever let a FALSE universal through, the interpreter would disagree at some
# instance. This is not the soundness theorem — it is bounded evidence for it, at the
# rung where a bug would be most dangerous.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
# gamma operational defs: plus, mult, and structural equality eqn (1 = equal, 0 = not)
DEFS='(def plus (a b) (match a (Ze b) ((Su x) (Su (plus x b))))) (def mult (a b) (match a (Ze Ze) ((Su x) (plus b (mult x b))))) (def eqn (a b) (match a (Ze (match b (Ze 1) (w 0))) ((Su x) (match b ((Su y) (eqn x y)) (w 0)))))'
# the five concrete instances k = 0..5 as Peano literals
K0=Ze; K1='(Su Ze)'; K2='(Su (Su Ze))'; K3='(Su (Su (Su Ze)))'; K4='(Su (Su (Su (Su Ze))))'; K5='(Su (Su (Su (Su (Su Ze)))))'

PASS=0; FAIL=0
# ind DESC  CHECKER_CERT  GAMMA_EXPR(with @ = k)  EXPECT(1 holds / 0 fails) per instance
ind() {
  if [ "$(printf '%s' "$2" | "$T/check.exe")" != accept ]; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta did not accept the inductive proof"; return; fi
  for k in "$K0" "$K1" "$K2" "$K3" "$K4" "$K5"; do
    expr=$(printf '%s' "$3" | sed "s/@/$k/g")
    printf '%s\n%s\n' "$DEFS" "$expr" | "$T/interp.exe" >/dev/null
    if [ "$?" != "$4" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : interpreter disagrees at k=$k"; return; fi
  done
  PASS=$((PASS+1))
}

# forall n. n + 0 = n   — proved by induction + eqelim; interp: plus(k,0) = k
ind "n + 0 = n" \
  "(All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))" \
  "(eqn (plus @ Ze) @)" 1
# forall n. n * 0 = 0   — induction (step by conversion); interp: mult(k,0) = 0
ind "n * 0 = 0" \
  "(All (= (m (v 0) z) z)) (natind (= (m (v 0) z) z) (refl z) (gen (lam (= (m (v 0) z) z) (hyp 0))))" \
  "(eqn (mult @ Ze) Ze)" 1
# forall n. n != s n    — induction + disj + sinj; interp: eqn(k, s k) = 0 (distinct)
ind "n != s n" \
  "(All (-> (= (v 0) (s (v 0))) (bot))) (natind (-> (= (v 0) (s (v 0))) (bot)) (lam (= z (s z)) (disj (hyp 0))) (gen (lam (-> (= (v 0) (s (v 0))) (bot)) (lam (= (s (v 0)) (s (s (v 0)))) (app (hyp 1) (sinj (hyp 0)))))))" \
  "(eqn @ (Su @))" 0

echo "induction soundness seam (inductive universals vs operational eval): $PASS confirmed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
