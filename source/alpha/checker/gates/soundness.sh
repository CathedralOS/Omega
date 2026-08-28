#!/usr/bin/env sh
# SOUNDNESS BATTERY — the trust anchor, attacked.
#
# proof_kernel.md's whole point: "this checker has the only authority — a false proposition
# cannot get past it, however the certificate was found." A proof-SEARCH engine is
# untrusted and may be buggy, adversarial, or backdoored; the checker is the one
# thing that must never accept a bad certificate. test.sh shows valid proofs are
# accepted; this script is the dual — a battery of INVALID certificates that must all
# be REJECTED. Every line here is a way someone might try to smuggle a falsehood
# past the checker; not one may succeed.
#
# Two kinds of attack:
#   (1) genuinely false propositions with their most natural (wrong) proof attempt;
#   (2) propositions that are TRUE CLASSICALLY but have no intuitionistic proof —
#       the checker is constructive (a proof is a program), so it must reject the
#       tempting classical "proofs". Accepting any of these would mean the checker
#       proves more than it should: a precision failure that is also a soundness one.
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
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }

PASS=0; FAIL=0
# no() — assert the certificate is REJECTED. An `accept` here is a breach.
no() { # description  "goal term"
  out=$(printf '%s' "$2" | "$T/check.exe")
  if [ "$out" = reject ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  BREACH ($out, expected reject): $1"; fi
}

# ---- (1) genuinely false / invalid certificates ----
no "P from nothing"              "P (hyp 0)"
no "Q from P"                    "(-> P Q) (lam P (hyp 0))"
no "out-of-scope hyp index"      "(-> P P) (lam P (hyp 1))"
no "and from one conjunct"       "(-> P (& P Q)) (lam P (pair (hyp 0) (hyp 0)))"
no "wrong projection"            "(-> (& P Q) Q) (lam (& P Q) (fst (hyp 0)))"
no "disjunction collapses"       "(-> (+ P Q) P) (lam (+ P Q) (case (hyp 0) (lam P (hyp 0)) (lam Q (hyp 0))))"
no "absurd without falsity"      "(-> P Q) (lam P (absurd Q (hyp 0)))"
no "bot from a true atom"        "(-> P (bot)) (lam P (hyp 0))"
no "apply a non-function"        "Q (app (hyp 0) (hyp 1))"
no "swap conclusion"             "(-> (-> P Q) (-> Q P)) (lam (-> P Q) (lam Q (app (hyp 1) (hyp 0))))"
# arithmetic falsehoods (the conversion rule must not be fooled)
no "2+2=5"                       "(= (p (s (s z)) (s (s z))) (s (s (s (s (s z))))))  (refl (s (s (s (s z)))))"
no "1+1=1"                       "(= (p (s z) (s z)) (s z))  (refl (s z))"
no "2*3=5"                       "(= (m (s (s z)) (s (s (s z)))) (s (s (s (s (s z))))))  (refl (s (s (s (s (s z))))))"
no "3*0=3"                       "(= (m (s (s (s z))) z) (s (s (s z))))  (refl (s (s (s z))))"
# first-order falsehoods
no "P(0) gives forall x.P(x)"    "(-> (Pred 0 z) (All (Pred 0 (v 0)))) (lam (Pred 0 z) (gen (hyp 0)))"
no "instantiation drifts pred"   "(-> (All (Pred 0 (v 0))) (Pred 1 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))"
no "variable capture"            "(All (-> (Pred 0 (v 0)) (All (Pred 0 (v 0))))) (gen (lam (Pred 0 (v 0)) (gen (hyp 0))))"
no "exists witness leaks out"    "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (Pred 0 (v 0)))) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (unpack (hyp 1) (hyp 0))))"
no "unpack C uses witness"       "(-> (Exists (Pred 0 (v 0))) (Pred 0 (v 0))) (lam (Exists (Pred 0 (v 0))) (unpack (hyp 0) (gen (lam (Pred 0 (v 0)) (hyp 0)))))"
no "fun: no rule stays stuck"    "(data 2 0 0 0) (data 4 0 0 0) (fun 7 2 z) (= (f 7 (k 4)) z) (refl (f 7 (k 4)))"
no "fun: open arg stays stuck"   "(data 2 0 0 0) (data 3 1 1 0) (fun 7 2 z) (fun 7 3 (s (rec 0))) (= (f 7 (v 0)) z) (refl (f 7 (v 0)))"
no "forall converse (false)"     "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 1 (v 0))) (All (Pred 0 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 1 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))"
no "induction w/o step"          "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))"
no "disj on a true eq"           "(-> (= (s z) (s z)) (bot)) (lam (= (s z) (s z)) (disj (hyp 0)))"
no "sinj fabricates eq"          "(-> (= (s z) (s z)) (= z (s z))) (lam (= (s z) (s z)) (sinj (hyp 0)))"

# ---- (2) classically true, but NOT intuitionistically provable ----
# the checker is constructive; these tempting classical "proofs" must all fail.
no "excluded middle"             "(+ P (-> P (bot))) (inr P (hyp 0))"
no "double-negation elim"        "(-> (-> (-> P (bot)) (bot)) P) (lam (-> (-> P (bot)) (bot)) (hyp 0))"
no "Peirce's law"                "(-> (-> (-> P Q) P) P) (lam (-> (-> P Q) P) (app (hyp 0) (lam P (hyp 0))))"
no "de Morgan (not-and)"         "(-> (-> (& P Q) (bot)) (+ (-> P (bot)) (-> Q (bot)))) (lam (-> (& P Q) (bot)) (inl (-> Q (bot)) (lam P (absurd (bot) (app (hyp 1) (pair (hyp 0) (hyp 0)))))))"
no "drinker (classical)"         "(Exists (-> (Pred 0 (v 0)) (All (Pred 0 (v 0))))) (wit (-> (Pred 0 (v 0)) (All (Pred 0 (v 0)))) z (lam (Pred 0 z) (gen (hyp 0))))"
no "gen captures var"            "(-> (Pred 0 (v 0)) (All (Pred 0 (v 0)))) (lam (Pred 0 (v 0)) (gen (hyp 0)))"
no "gen captures (eq)"           "(-> (= (v 0) z) (All (= (v 0) z))) (lam (= (v 0) z) (gen (hyp 0)))"

# list membership Mem(x,L) = (Rel 777 x L): the inductive predicate must not let a FALSE
# membership (or falsity) be proved closed — its only closed sources are the two intros.
no "mem in empty list"           "(Rel 777 (s z) nil) (memtail (s z) (memhead (s z) nil))"
no "mem head fakes nil"          "(Rel 777 (s (s z)) nil) (memhead (s (s z)) nil)"
no "mem wrong element"           "(Rel 777 (s (s z)) (cons (s z) nil)) (memhead (s (s z)) nil)"
no "falsity via memnil on cons"  "(bot) (memnil (memhead (s z) nil))"
no "memtail changes element"     "(Rel 777 (s (s z)) (cons (s z) (cons (s (s (s z))) nil))) (memtail (s z) (memhead (s (s (s z))) nil))"

no "prodis fakes product"        "(Rel 778 nil (s (s z))) (pnil)"
no "prodis cons wrong product"   "(Rel 778 (cons (s (s z)) nil) (s (s (s (s (s z)))))) (pcons (s (s z)) (pnil))"

no "prodnilinv on a cons"        "(= (m (s (s z)) (s z)) (s z)) (prodnilinv (pcons (s (s z)) (pnil)))"
no "prodconsinv on nil"        "(All (Exists (& (= (s z) (m (v 1) (v 0))) (Rel 778 nil (v 0))))) (gen (prodconsinv (pnil)))"

no "prodconsinv wrong product order"  "(All (All (All (-> (Rel 778 (cons (v 2) (v 1)) (v 0)) (Exists (& (= (v 1) (m (v 0) (v 3))) (Rel 778 (v 2) (v 0)))))))) (gen (gen (gen (lam (Rel 778 (cons (v 2) (v 1)) (v 0)) (prodconsinv (hyp 0))))))"
no "prodconsinv on a nil"             "(All (-> (Rel 778 nil (v 0)) (Exists (& (= (v 1) (m (v 1) (v 0))) (Rel 778 nil (v 0)))))) (gen (lam (Rel 778 nil (v 0)) (prodconsinv (hyp 0))))"

echo "soundness battery (invalid certificates must all be rejected): $PASS rejected, $FAIL breached"
[ "$FAIL" = 0 ] || exit 1
