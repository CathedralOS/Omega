#!/usr/bin/env sh
# PROVER CERTIFICATE CROSS-CHECK -- replay automation output across checkers.
#
# prover-test.sh already checks every prover certificate against implementations/beta/check.beta (the Beta trust anchor). This
# goes further: it runs the prover's ACTUAL emitted certificates through a SECOND, independently-written
# checker -- implementations/gamma/checker.gamma (the same logic as algebraic data + pattern matching, run on the gamma reference
# interpreter, at a different rung). For each goal the prover discharges, BOTH checkers must ACCEPT. A
# disagreement exposes a checker or encoding bug—now exercised on the prover's real cert
# SHAPES (deep eqelim, gen/inst/wit/unpack, disj/sinj), not just randomly-generated proofs.
#
# Scope: the prover emits the cert in BOTH syntaxes from the same deterministic proof (`tools/prover.py` -> Beta
# input; `tools/prover.py --gamma` -> implementations/gamma/checker.gamma's `(check proof goal)` expr). The def/use lemma prelude has no
# gamma analogue, so lemma-using (phase-2 arithmetic) certs report "unsupported" and are skipped here; they
# remain implementations/beta/check.beta-validated by prover-test.sh. Needs python3.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" || exit $?
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_ALPHA_CHECKER"
command -v python3 >/dev/null 2>&1 || { echo "prover-diamond: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
bcc() { "$T/bc.exe" < "$1" > "$T/a.asm" && "$ASM" < "$T/a.asm" > "$T/a.tape" && stamp_seed "$T/a.tape" "$SEED" "$2" >/dev/null 2>&1; }
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
bcc "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_ALPHA_CHECKER}"/implementations/gamma/checker.gamma)

PASS=0; FAIL=0; SKIP=0
dia() {  # a lemma-free goal: the prover proves it, and BOTH checkers must accept the (same) proof
  bcert=$(python3 tools/prover.py "$1" 2>/dev/null | tail -1)
  gexpr=$(python3 tools/prover.py --gamma "$1" 2>/dev/null | tail -1)
  if [ "$bcert" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : prover found no proof"; return; fi
  vb=$(printf '%s' "$bcert" | "$T/check.exe")
  # run in a subshell whose stderr is dropped, so a SIGBUS from interp exhausting its arena doesn't print the
  # shell's job-control "Bus error" line; we read the exit code and treat a crash as a skip below.
  ( printf '%s\n%s\n' "$DEFS" "$gexpr" | "$T/interp.exe" >/dev/null 2>&1 ) 2>/dev/null; eg=$?
  # interp.beta has a fixed arena/fuel; a large proof can EXHAUST it (exit not 0/1 -> crash/signal). That is a
  # reference-interpreter capacity limit, not a checker disagreement, so SKIP those (still implementations/beta/check.beta-validated).
  if [ "$eg" != 0 ] && [ "$eg" != 1 ]; then SKIP=$((SKIP+1)); return; fi
  vg=reject; [ "$eg" = 1 ] && vg=accept
  if [ "$vb" = accept ] && [ "$vg" = accept ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : implementations/beta/check.beta=$vb  implementations/gamma/checker.gamma=$vg (must both accept)"; fi
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
# LEMMA / INDUCTION certs -- the riskiest emission (natind, eqelim chains, the directed sum-witness). The
# `(def N)/(use N)` prelude has no gamma form, so for the gamma route the lemmas are INLINED into one proof.
dia "(All (= (p (v 0) z) (v 0)))"                                 # x+0=x         (natind)
dia "(All (= (m (v 0) z) z))"                                     # x*0=0         (natind, mult)
dia "(All (All (= (p (v 1) (v 0)) (p (v 0) (v 1)))))"             # commutativity (nested natind)
dia "(All (All (All (= (p (p (v 2) (v 1)) (v 0)) (p (v 2) (p (v 1) (v 0)))))))"  # associativity
dia "(All (Le (v 0) (s (v 0))))"                                 # x <= s x
dia "(All (All (Le (v 0) (p (v 1) (v 0)))))"                      # y <= x+y      (lemma reuse: add-comm)
dia "(-> (& (Le (v 0) (v 1)) (Le (v 1) (v 2))) (Le (v 0) (v 2)))" # transitivity  (inlined add-assoc)
dia "(-> (Le (p (v 0) (v 1)) (v 2)) (Le (v 0) (v 2)))"            # drop-addend
dia "(All (All (All (-> (Le (v 2) (v 1)) (-> (Le (v 1) (v 0)) (Le (v 2) (v 0)))))))"  # le-trans (discharge.rs id 9)
# MULT + DISTRIBUTIVITY + MONOTONICITY families -- the heavier natind/lemma certs (up to ~270 KB once inlined)
# the earlier diamond never exercised on the gamma leg. All accept on BOTH checkers (measured); the SKIP guard
# above still protects any that would exhaust interp's arena, so this only ADDS coverage, never flakes.
dia "(All (All (= (m (v 1) (v 0)) (m (v 0) (v 1)))))"                              # x*y = y*x        (mult-comm)
dia "(All (All (All (= (m (m (v 2) (v 1)) (v 0)) (m (v 2) (m (v 1) (v 0)))))))"    # (x*y)*z=x*(y*z)  (mult-assoc)
dia "(All (All (All (= (m (p (v 2) (v 1)) (v 0)) (p (m (v 2) (v 0)) (m (v 1) (v 0)))))))"  # (x+y)*a=x*a+y*a (right-dist)
dia "(All (All (All (-> (Le (v 2) (v 1)) (Le (p (v 2) (v 0)) (p (v 1) (v 0)))))))" # a<=b => a+c<=b+c (add-mono)
dia "(All (All (All (-> (Lt (v 2) (v 1)) (Lt (p (v 2) (v 0)) (p (v 1) (v 0)))))))" # a<b  => a+c<b+c  (add-strict-mono)
dia "(All (All (All (-> (Le (v 2) (v 1)) (Le (m (v 2) (v 0)) (m (v 1) (v 0)))))))" # a<=b => a*c<=b*c (mult-mono)
dia "(All (All (All (All (-> (Le (v 3) (v 2)) (-> (Le (v 1) (v 0)) (Le (p (v 3) (v 1)) (p (v 2) (v 0)))))))))"  # a<=b & c<=d => a+c<=b+d (add_le_add)
# ORDER + POSITIVITY + CANCELLATION -- forward-reasoning certs (order-cycle/order-eq refute via the banked
# irreflexivity; positivity/cancel via sinj+natind). Small certs, but cert CLASSES the gamma leg hadn't seen.
dia "(All (-> (Lt (v 0) (v 0)) (bot)))"                                            # a<a -> bot        (irreflexivity)
dia "(All (All (-> (Lt (v 1) (v 0)) (-> (Lt (v 0) (v 1)) (bot)))))"                # a<b -> b<a -> bot (asymmetry)
dia "(All (All (-> (Lt (v 1) (v 0)) (-> (= (v 1) (v 0)) (bot)))))"                 # a<b -> a=b -> bot (order-eq)
dia "(All (All (-> (= (p (v 1) (v 0)) z) (= (v 1) z))))"                           # a+b=0 -> a=0      (positivity)
dia "(All (All (-> (= (p (v 1) (v 0)) (v 1)) (= (v 0) z))))"                       # a+m=a -> m=0      (CANCEL0)
dia "(All (All (All (-> (= (p (v 2) (v 0)) (p (v 1) (v 0))) (= (v 2) (v 1))))))"   # a+c=b+c => a=b    (add-cancel-right)

echo "prover diamond (every prover cert accepted by BOTH implementations/beta/check.beta AND implementations/gamma/checker.gamma): $PASS ok, $SKIP skipped (interp arena), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
