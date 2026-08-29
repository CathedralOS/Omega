#!/usr/bin/env sh
# Compact discriminator suite for the authoritative Alpha-tape checker.
# This is rule coverage, not a theorem library: each retained case distinguishes
# an implemented calculus boundary. Broad differential coverage lives in the
# single independent reference diamond.
set -u

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh"
cd "$OMEGA_PATH_ALPHA_CHECKER"

TMP=$(mktemp -d)
trap 'trash "$TMP"' EXIT
stamp_proof_checker "$TMP/check" >/dev/null

PASS=0
FAIL=0
chk() { # description certificate expected-outcome
  out=$(printf '%s' "$2" | "$TMP/check")
  if [ "$out" = "$3" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "FAIL: $1 — expected $3, got '$out'"
  fi
}

# Propositional natural deduction and resource boundaries.
chk "implication introduction" "(-> P P) (lam P (hyp 0))" accept
chk "unbound hypothesis" "P (hyp 0)" reject
chk "implication elimination" "(-> (& (-> P Q) P) Q) (lam (& (-> P Q) P) (app (fst (hyp 0)) (snd (hyp 0))))" accept
chk "pair projection mismatch" "(-> (& P Q) Q) (lam (& P Q) (fst (hyp 0)))" reject
chk "sum elimination" "(-> (+ P Q) (+ Q P)) (lam (+ P Q) (case (hyp 0) (lam P (inr Q (hyp 0))) (lam Q (inl P (hyp 0)))))" accept
chk "sum branches disagree" "(-> (+ P Q) P) (lam (+ P Q) (case (hyp 0) (lam P (hyp 0)) (lam Q (hyp 0))))" reject
chk "falsity elimination" "(-> (bot) P) (lam (bot) (absurd P (hyp 0)))" accept
chk "falsity requires bottom" "(-> P Q) (lam P (absurd Q (hyp 0)))" reject

# Definitional equality and first-order binding.
chk "computed reflexivity" "(= (p (s z) (s z)) (s (s z))) (refl (s (s z)))" accept
chk "false computation" "(= (p (s z) (s z)) (s z)) (refl (s z))" reject
chk "universal introduction" "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (gen (lam (Pred 0 (v 0)) (hyp 0)))" accept
chk "universal instantiation" "(-> (All (Pred 0 (v 0))) (Pred 0 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))" accept
chk "generalization capture" "(-> (Pred 0 z) (All (Pred 0 (v 0)))) (lam (Pred 0 z) (gen (hyp 0)))" reject
chk "existential introduction" "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) z (hyp 0)))" accept
chk "existential witness mismatch" "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) (s z) (hyp 0)))" reject
chk "relation argument order" "(-> (Rel 0 z (s z)) (Rel 0 (s z) z)) (lam (Rel 0 z (s z)) (hyp 0))" reject

# Induction, equality transport, and no-confusion.
chk "natural induction" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "natural induction needs successor step" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
chk "equality transport" "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z)))) (lam (Pred 0 (p (s z) (s z))) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" accept
chk "transport mismatch" "(-> (Pred 0 (s z)) (Pred 0 (s (s z)))) (lam (Pred 0 (s z)) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" reject
chk "zero/successor disjoint" "(-> (= z (s z)) (bot)) (lam (= z (s z)) (disj (hyp 0)))" accept
chk "successor injective" "(-> (= (s (v 0)) (s z)) (= (v 0) z)) (lam (= (s (v 0)) (s z)) (sinj (hyp 0)))" accept

# Lists and user-declared structural recursion.
chk "append computation" "(= (app (cons z nil) (cons (s z) nil)) (cons z (cons (s z) nil))) (refl (cons z (cons (s z) nil)))" accept
chk "list induction" "(-> (Pred 0 nil) (-> (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 nil) (lam (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (listind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "constructor structure" "(= (k 1 (k 0) (k 0)) (k 1 (k 0) (k 0))) (refl (k 1 (k 0) (k 0)))" accept
chk "constructor mismatch" "(= (k 0) (k 1 (k 0) (k 0))) (refl (k 0))" reject
chk "declared structural induction" "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "structural induction needs every IH" "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject

# Ground user functions and checked named lemmas.
FG="(data 2 0 0 0) (data 3 1 1 0) (fun 7 2 z) (fun 7 3 (s (rec 0)))"
chk "user function reduction" "$FG (= (f 7 (k 3 (k 2))) (s z)) (refl (f 7 (k 3 (k 2))))" accept
chk "user function wrong result" "$FG (= (f 7 (k 3 (k 2))) (s (s z))) (refl (f 7 (k 3 (k 2))))" reject
chk "open user function remains stuck" "$FG (= (f 7 (v 0)) z) (refl (f 7 (v 0)))" reject
chk "checked lemma use" "(def 0 (-> P P) (lam P (hyp 0))) (-> P P) (use 0)" accept
chk "invalid lemma definition" "(def 0 (-> P Q) (lam P (hyp 0))) (-> P Q) (use 0)" reject

# The two retained inductive predicates: membership and product witnesses.
chk "membership head" "(Rel 777 (s z) (cons (s z) nil)) (memhead (s z) nil)" accept
chk "membership cannot fabricate nil" "(Rel 777 (s z) nil) (memhead (s z) nil)" reject
chk "product witness" "(Rel 778 (cons (s (s z)) nil) (m (s (s z)) (s z))) (pcons (s (s z)) (pnil))" accept
chk "product witness result mismatch" "(Rel 778 nil (s (s z))) (pnil)" reject
chk "product nil inversion" "(All (-> (Rel 778 nil (v 0)) (= (v 0) (s z)))) (gen (lam (Rel 778 nil (v 0)) (prodnilinv (hyp 0))))" accept

echo "checker rule discriminators: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
