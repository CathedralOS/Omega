#!/usr/bin/env sh
# PROVER DIAMOND -- the diversity-security thesis applied to the proof-AUTOMATION front line.
#
# prover-test.sh already checks every prover certificate against check.beta (the Beta trust anchor). This
# goes further: it runs the prover's ACTUAL emitted certificates through a SECOND, independently-written
# checker -- checker.gamma (the same logic as algebraic data + pattern matching, run on the gamma reference
# interpreter, at a different rung). For each goal the prover discharges, BOTH checkers must ACCEPT. A
# disagreement would expose a bug (or a backdoor) in one checker -- now exercised on the prover's real cert
# SHAPES (deep eqelim, gen/inst/wit/unpack, disj/sinj), not just randomly-generated proofs.
#
# Scope: the prover emits the cert in BOTH syntaxes from the same deterministic proof (`prover.py` -> Beta
# input; `prover.py --gamma` -> checker.gamma's `(check proof goal)` expr). The def/use lemma prelude has no
# gamma analogue, so lemma-using (phase-2 arithmetic) certs report "unsupported" and are skipped here; they
# remain check.beta-validated by prover-test.sh. Needs python3.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "prover-diamond: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
bcc() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/a.asm" && "$ASM" < "$T/a.asm" > "$T/a.tape" && stamp_seed "$T/a.tape" "$SEED" "$2" >/dev/null 2>&1; }
bcc check.beta "$T/check.exe"           || { echo "build check.beta failed"; exit 1; }
bcc ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
DEFS=$(cat ../gamma/checker.gamma)

PASS=0; FAIL=0; SKIP=0
dia() {  # a lemma-free goal: the prover proves it, and BOTH checkers must accept the (same) proof
  bcert=$(python3 prover.py "$1" 2>/dev/null | tail -1)
  gexpr=$(python3 prover.py --gamma "$1" 2>/dev/null | tail -1)
  if [ "$bcert" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : prover found no proof"; return; fi
  if [ "$gexpr" = unsupported ]; then SKIP=$((SKIP+1)); return; fi   # lemma cert: no gamma analogue
  vb=$(printf '%s' "$bcert" | "$T/check.exe")
  printf '%s\n%s\n' "$DEFS" "$gexpr" | "$T/interp.exe" >/dev/null; eg=$?
  vg=reject; [ "$eg" = 1 ] && vg=accept
  if [ "$vb" = accept ] && [ "$vg" = accept ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : check.beta=$vb  checker.gamma=$vg (must both accept)"; fi
}

# propositional -- every connective + its intro/elim (lam/app, pair/fst/snd, inl/inr/case, absurd)
dia "(-> P P)"
dia "(-> (& P Q) P)"
dia "(-> (& P Q) (& Q P))"
dia "(-> (& (-> P Q) P) Q)"
dia "(-> P (-> (-> P Q) Q))"
dia "(-> P (+ P Q))"
dia "(-> (+ P Q) (+ Q P))"
dia "(-> (& P (+ Q R)) (+ (& P Q) (& P R)))"
dia "(-> (& (-> P R) (-> Q R)) (-> (+ P Q) R))"
dia "(-> (bot) P)"
# first-order -- gen/inst/wit/unpack over a uniform eigenvariable scheme
dia "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))"
dia "(-> (All (Pred 0 (v 0))) (Pred 0 (s z)))"
dia "(-> (Pred 0 (s z)) (Exists (Pred 0 (v 0))))"
dia "(-> (All (Pred 0 (v 0))) (Exists (Pred 0 (v 0))))"
dia "(All (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (v 0)))))"
dia "(-> (Exists (& (Pred 0 (v 0)) (Pred 1 (v 0)))) (Exists (Pred 0 (v 0))))"
dia "(-> (& (Exists (Pred 0 (v 0))) (All (-> (Pred 0 (v 0)) (Pred 1 (v 0))))) (Exists (Pred 1 (v 0))))"
# equality + Peano discrimination -- refl, eqelim (sym/trans/congruence/transport), disj, sinj
dia "(= (p (s z) (s z)) (s (s z)))"
dia "(-> (= (s z) (s (s z))) (= (s (s z)) (s z)))"
dia "(-> (& (= (s z) (s (s z))) (= (s (s z)) z)) (= (s z) z))"
dia "(-> (& (Pred 0 (s z)) (= (s z) (s (s z)))) (Pred 0 (s (s z))))"
dia "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z))))"
dia "(-> (= z (s z)) (bot))"
dia "(-> (= (s (s z)) (s (v 0))) (= (s z) (v 0)))"
# inequality -- the lemma-free weakenings (unpack/wit on the desugared existential)
dia "(-> (Lt (v 0) (v 1)) (Le (v 0) (v 1)))"
dia "(Lt (s z) (s (s (s z))))"

echo "prover diamond (every prover cert accepted by BOTH check.beta AND checker.gamma): $PASS ok, $SKIP skipped (lemma certs), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
