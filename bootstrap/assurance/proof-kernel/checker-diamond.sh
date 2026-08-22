#!/usr/bin/env sh
# CHECKER CROSS-CHECK — replay the corpus across checker implementations.
#
# Separate implementations are useful bug-finding evidence while the formal
# soundness bridge matures. We have two: check.beta (Beta; term/type trees hand-encoded as
# tagged memory nodes, decided by integer-tag if-cascades) and checker.gamma
# (Gamma; the same logic as algebraic data + pattern matching, run on the gamma
# reference interpreter). They were written differently, in different languages,
# at different rungs. For each proof below — expressed in BOTH input syntaxes — the
# two checkers must return the SAME verdict, and it must be the expected one. A
# disagreement exposes a bug or unsupported semantic mismatch. Agreement is not
# DDC and does not itself prove either checker sound.
#
# A THIRD oracle joins below: checker_typed.gamma — the fully type-annotated checker
# that typeck.beta accepts — mechanically type-erased (erase_types.py) to the untyped
# surface interp runs. It must agree with checker.gamma on ALL 83 cases (user-function
# proofs are rewritten from the wrapper rule form to the typed flat form by
# frule_to_flat.py). This makes "the checker is statically type-safe" and "the checker is
# behaviorally correct" claims about the SAME artifact, rather than two copies kept in sync
# by hand.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
bcc() { "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$1" > "$T/a.asm" && "$ASM" < "$T/a.asm" > "$T/a.tape" && stamp_seed "$T/a.tape" "$SEED" "$2" >/dev/null 2>&1; }
bcc check.beta "$T/check.exe"          || { echo "build check.beta failed"; exit 1; }
bcc "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
DEFS=$(cat "${OMEGA_PATH_GAMMA}"/checker.gamma)
# Third oracle: the TYPE-CHECKED checker (checker_typed.gamma, the artifact typeck.beta
# accepts), mechanically type-erased to the untyped surface interp runs. Agreement here
# means the checker gamma's type system validates is the SAME checker that's behaviorally
# trusted — not a second, hand-kept-in-sync copy. Guarded on python3 (the eraser), exactly
# like the elaborator tools in verify-lattice.sh; without python3 the two-checker diamond
# is unchanged. The user-function cases (Fapp/Frule) encode rules in checker.gamma's
# WRAPPER form ((Frule ca ba)); frule_to_flat.py rewrites those to the typed checker's
# FLAT form ((Fapp arg ca ba cb bb)) so they cross-check too — exercising fdisp, the one
# helper where the two representations actually diverge. No case is skipped.
TPASS=0; TFAIL=0; HAVE_TYPED=0
if command -v python3 >/dev/null 2>&1; then
  if python3 "${OMEGA_PATH_GAMMA}"/erase_types.py < "${OMEGA_PATH_GAMMA}"/checker_typed.gamma > "$T/erased.gamma"; then
    TDEFS=$(cat "$T/erased.gamma"); HAVE_TYPED=1
  fi
fi

PASS=0; FAIL=0
# dia DESC  BETA_INPUT  GAMMA_CHECK_EXPR  EXPECT(accept|reject)
dia() {
  vb=$(printf '%s' "$2" | "$T/check.exe")                       # check.beta -> accept/reject
  printf '%s\n%s\n' "$DEFS" "$3" | "$T/interp.exe" >/dev/null; vg=$?   # gamma -> 1/0
  gv=reject; [ "$vg" = 1 ] && gv=accept
  if [ "$vb" = "$gv" ] && [ "$vb" = "$4" ]; then PASS=$((PASS+1))
  else FAIL=$((FAIL+1)); echo "  FAIL $1 : beta=$vb gamma=$gv expect=$4"; fi
  if [ "$HAVE_TYPED" = 1 ]; then
    case "$3" in
      *Fapp*|*Frule*) t3=$(printf '%s' "$3" | python3 "${OMEGA_PATH_GAMMA}"/frule_to_flat.py) ;;  # wrapper rules -> flat
      *) t3=$3 ;;
    esac
    printf '%s\n%s\n' "$TDEFS" "$t3" | "$T/interp.exe" >/dev/null; vt=$?
    if [ "$vt" = "$vg" ]; then TPASS=$((TPASS+1))
    else TFAIL=$((TFAIL+1)); echo "  FAIL(typed) $1 : checker.gamma=$vg type-erased=$vt"; fi
  fi
}
#    desc            check.beta syntax                                          checker.gamma syntax                                                                   expect
dia "identity"       "(-> P P) (lam P (hyp 0))"                                  "(check (Lam (Atom 0) (Hyp 0)) (Arrow (Atom 0) (Atom 0)))"                              accept
dia "wrong goal"     "(-> P Q) (lam P (hyp 0))"                                  "(check (Lam (Atom 0) (Hyp 0)) (Arrow (Atom 0) (Atom 1)))"                              reject
dia "and-elim"       "(-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))"                "(check (Lam (And (Atom 0) (Atom 1)) (Fst (Hyp 0))) (Arrow (And (Atom 0) (Atom 1)) (Atom 0)))" accept
dia "mismatch"       "(-> (& P Q) Q) (lam (& P Q) (fst (hyp 0)))"                "(check (Lam (And (Atom 0) (Atom 1)) (Fst (Hyp 0))) (Arrow (And (Atom 0) (Atom 1)) (Atom 1)))" reject
dia "modus ponens"   "(-> (& (-> P Q) P) Q) (lam (& (-> P Q) P) (app (fst (hyp 0)) (snd (hyp 0))))" "(check (Lam (And (Arrow (Atom 0) (Atom 1)) (Atom 0)) (App (Fst (Hyp 0)) (Snd (Hyp 0)))) (Arrow (And (Arrow (Atom 0) (Atom 1)) (Atom 0)) (Atom 1)))" accept
dia "or-commute"     "(-> (+ P Q) (+ Q P)) (lam (+ P Q) (case (hyp 0) (lam P (inr Q (hyp 0))) (lam Q (inl P (hyp 0)))))" "(check (Lam (Or (Atom 0) (Atom 1)) (Case (Hyp 0) (Lam (Atom 0) (Inr (Atom 1) (Hyp 0))) (Lam (Atom 1) (Inl (Atom 0) (Hyp 0))))) (Arrow (Or (Atom 0) (Atom 1)) (Or (Atom 1) (Atom 0))))" accept
dia "ex falso"       "(-> (bot) P) (lam (bot) (absurd P (hyp 0)))"               "(check (Lam Bot (Absurd (Atom 0) (Hyp 0))) (Arrow Bot (Atom 0)))"                      accept
# intuitionistic negation theorems (¬A = A -> ⊥): both checkers must agree
dia "non-contradict" "(-> (& A (-> A (bot))) (bot)) (lam (& A (-> A (bot))) (app (snd (hyp 0)) (fst (hyp 0))))" "(check (Lam (And (Atom 0) (Arrow (Atom 0) Bot)) (App (Snd (Hyp 0)) (Fst (Hyp 0)))) (Arrow (And (Atom 0) (Arrow (Atom 0) Bot)) Bot))" accept
dia "double-neg-in"  "(-> A (-> (-> A (bot)) (bot))) (lam A (lam (-> A (bot)) (app (hyp 0) (hyp 1))))" "(check (Lam (Atom 0) (Lam (Arrow (Atom 0) Bot) (App (Hyp 0) (Hyp 1)))) (Arrow (Atom 0) (Arrow (Arrow (Atom 0) Bot) Bot)))" accept
dia "contrapositive" "(-> (-> A B) (-> (-> B (bot)) (-> A (bot)))) (lam (-> A B) (lam (-> B (bot)) (lam A (app (hyp 1) (app (hyp 2) (hyp 0))))))" "(check (Lam (Arrow (Atom 0) (Atom 1)) (Lam (Arrow (Atom 1) Bot) (Lam (Atom 0) (App (Hyp 1) (App (Hyp 2) (Hyp 0)))))) (Arrow (Arrow (Atom 0) (Atom 1)) (Arrow (Arrow (Atom 1) Bot) (Arrow (Atom 0) Bot))))" accept
dia "triple-neg"     "(-> (-> (-> (-> A (bot)) (bot)) (bot)) (-> A (bot))) (lam (-> (-> (-> A (bot)) (bot)) (bot)) (lam A (app (hyp 1) (lam (-> A (bot)) (app (hyp 0) (hyp 1))))))" "(check (Lam (Arrow (Arrow (Arrow (Atom 0) Bot) Bot) Bot) (Lam (Atom 0) (App (Hyp 1) (Lam (Arrow (Atom 0) Bot) (App (Hyp 0) (Hyp 1)))))) (Arrow (Arrow (Arrow (Arrow (Atom 0) Bot) Bot) Bot) (Arrow (Atom 0) Bot)))" accept
dia "de Morgan ->"   "(-> (& (-> A (bot)) (-> B (bot))) (-> (+ A B) (bot))) (lam (& (-> A (bot)) (-> B (bot))) (lam (+ A B) (case (hyp 0) (lam A (app (fst (hyp 2)) (hyp 0))) (lam B (app (snd (hyp 2)) (hyp 0))))))" "(check (Lam (And (Arrow (Atom 0) Bot) (Arrow (Atom 1) Bot)) (Lam (Or (Atom 0) (Atom 1)) (Case (Hyp 0) (Lam (Atom 0) (App (Fst (Hyp 2)) (Hyp 0))) (Lam (Atom 1) (App (Snd (Hyp 2)) (Hyp 0)))))) (Arrow (And (Arrow (Atom 0) Bot) (Arrow (Atom 1) Bot)) (Arrow (Or (Atom 0) (Atom 1)) Bot)))" accept
dia "excl-middle no" "(+ A (-> A (bot))) (inl (-> A (bot)) (hyp 0))"             "(check (Inl (Arrow (Atom 0) Bot) (Hyp 0)) (Or (Atom 0) (Arrow (Atom 0) Bot)))"          reject
dia "unbound hyp"    "P (hyp 0)"                                                 "(check (Hyp 0) (Atom 0))"                                                              reject
dia "refl 2+2=4"     "(= (p (s (s z)) (s (s z))) (s (s (s (s z)))))  (refl (s (s (s (s z)))))" "(check (Refl (Su (Su (Su (Su Ze))))) (Eq (Pl (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su Ze))))))" accept
dia "reject 2+2=5"   "(= (p (s (s z)) (s (s z))) (s (s (s (s (s z))))))  (refl (s (s (s (s z)))))" "(check (Refl (Su (Su (Su (Su Ze))))) (Eq (Pl (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su (Su Ze)))))))" reject
# first-order predicate logic: ∀ (gen/inst) and ∃ (wit/unpack)
dia "forall-intro"   "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (gen (lam (Pred 0 (v 0)) (hyp 0)))" "(check (Gen (Lam (Pred 0 (Iv 0)) (Hyp 0))) (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0)))))" accept
dia "forall-elim"    "(-> (All (Pred 0 (v 0))) (Pred 0 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))" "(check (Lam (All (Pred 0 (Iv 0))) (Inst (Hyp 0) Ze)) (Arrow (All (Pred 0 (Iv 0))) (Pred 0 Ze)))" accept
dia "P0 not forall"  "(-> (Pred 0 z) (All (Pred 0 (v 0)))) (lam (Pred 0 z) (gen (hyp 0)))" "(check (Lam (Pred 0 Ze) (Gen (Hyp 0))) (Arrow (Pred 0 Ze) (All (Pred 0 (Iv 0)))))" reject
dia "exists-intro"   "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) z (hyp 0)))" "(check (Lam (Pred 0 Ze) (Wit (Pred 0 (Iv 0)) Ze (Hyp 0))) (Arrow (Pred 0 Ze) (Exists (Pred 0 (Iv 0)))))" accept
dia "exists-elim"    "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 0 (v 0)) Q)) Q)) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 0 (v 0)) Q)) (unpack (hyp 1) (hyp 0))))" "(check (Lam (Exists (Pred 0 (Iv 0))) (Lam (All (Arrow (Pred 0 (Iv 0)) (Atom 9))) (Unpack (Hyp 1) (Hyp 0)))) (Arrow (Exists (Pred 0 (Iv 0))) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Atom 9))) (Atom 9))))" accept
dia "witness leak"   "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (Pred 0 (v 0)))) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (unpack (hyp 1) (hyp 0))))" "(check (Lam (Exists (Pred 0 (Iv 0))) (Lam (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0)))) (Unpack (Hyp 1) (Hyp 0)))) (Arrow (Exists (Pred 0 (Iv 0))) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0)))) (Pred 0 (Iv 0)))))" reject
# quantifier interchange ∃∀ -> ∀∃ (unpack admits a C mentioning the OUTER var); both agree
dia "exists-forall swap" "(-> (Exists (All (Rel 0 (v 0) (v 1)))) (All (Exists (Rel 0 (v 1) (v 0))))) (lam (Exists (All (Rel 0 (v 0) (v 1)))) (gen (unpack (hyp 0) (gen (lam (All (Rel 0 (v 0) (v 1))) (wit (Rel 0 (v 2) (v 0)) (v 0) (inst (hyp 0) (v 1))))))))" "(check (Lam (Exists (All (Rel 0 (Iv 0) (Iv 1)))) (Gen (Unpack (Hyp 0) (Gen (Lam (All (Rel 0 (Iv 0) (Iv 1))) (Wit (Rel 0 (Iv 2) (Iv 0)) (Iv 0) (Inst (Hyp 0) (Iv 1)))))))) (Arrow (Exists (All (Rel 0 (Iv 0) (Iv 1)))) (All (Exists (Rel 0 (Iv 1) (Iv 0))))))" accept
dia "unpack wit leak" "(-> (Exists (Pred 0 (v 0))) (Pred 0 (v 0))) (lam (Exists (Pred 0 (v 0))) (unpack (hyp 0) (gen (lam (Pred 0 (v 0)) (hyp 0)))))" "(check (Lam (Exists (Pred 0 (Iv 0))) (Unpack (Hyp 0) (Gen (Lam (Pred 0 (Iv 0)) (Hyp 0))))) (Arrow (Exists (Pred 0 (Iv 0))) (Pred 0 (Iv 0))))" reject
dia "exists over +"  "(-> (Exists (+ (Pred 0 (v 0)) (Pred 1 (v 0)))) (+ (Exists (Pred 0 (v 0))) (Exists (Pred 1 (v 0))))) (lam (Exists (+ (Pred 0 (v 0)) (Pred 1 (v 0)))) (unpack (hyp 0) (gen (lam (+ (Pred 0 (v 0)) (Pred 1 (v 0))) (case (hyp 0) (lam (Pred 0 (v 0)) (inl (Exists (Pred 1 (v 0))) (wit (Pred 0 (v 0)) (v 0) (hyp 0)))) (lam (Pred 1 (v 0)) (inr (Exists (Pred 0 (v 0))) (wit (Pred 1 (v 0)) (v 0) (hyp 0)))))))))" "(check (Lam (Exists (Or (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Unpack (Hyp 0) (Gen (Lam (Or (Pred 0 (Iv 0)) (Pred 1 (Iv 0))) (Case (Hyp 0) (Lam (Pred 0 (Iv 0)) (Inl (Exists (Pred 1 (Iv 0))) (Wit (Pred 0 (Iv 0)) (Iv 0) (Hyp 0)))) (Lam (Pred 1 (Iv 0)) (Inr (Exists (Pred 0 (Iv 0))) (Wit (Pred 1 (Iv 0)) (Iv 0) (Hyp 0))))))))) (Arrow (Exists (Or (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Or (Exists (Pred 0 (Iv 0))) (Exists (Pred 1 (Iv 0))))))" accept
# user-defined recursive FUNCTIONS — check.beta stores rules in a (fid,cid) table from a
# (fun …) decl prefix; checker.gamma carries the SAME rules INLINE on the Fapp node. Both
# must compute g(S Z)=s z (g embeds user-Nat into Peano: g Z = z, g(S x)=s(g x)).
dia "fun g(S Z)=s z" "(data 2 0 0 0) (data 3 1 1 0) (fun 7 2 z) (fun 7 3 (s (rec 0))) (= (f 7 (k 3 (k 2))) (s z)) (refl (f 7 (k 3 (k 2))))" "(check (Refl (Fapp (Apply (Con 3) (Con 2)) (Frule 2 Ze) (Frule 3 (Su (Reccall 0))))) (Eq (Fapp (Apply (Con 3) (Con 2)) (Frule 2 Ze) (Frule 3 (Su (Reccall 0)))) (Su Ze)))" accept
dia "fun g(S Z)!=2"  "(data 2 0 0 0) (data 3 1 1 0) (fun 7 2 z) (fun 7 3 (s (rec 0))) (= (f 7 (k 3 (k 2))) (s (s z))) (refl (f 7 (k 3 (k 2))))" "(check (Refl (Fapp (Apply (Con 3) (Con 2)) (Frule 2 Ze) (Frule 3 (Su (Reccall 0))))) (Eq (Fapp (Apply (Con 3) (Con 2)) (Frule 2 Ze) (Frule 3 (Su (Reccall 0)))) (Su (Su Ze))))" reject
# arity 2: a binary Tree (Leaf=4, Node=5), sz = leaf-count recursing on both children
dia "fun Tree sz=3"  "(data 4 0 0 0) (data 5 2 1 1) (fun 8 4 (s z)) (fun 8 5 (p (rec 0) (rec 1))) (= (f 8 (k 5 (k 4) (k 5 (k 4) (k 4)))) (s (s (s z)))) (refl (f 8 (k 5 (k 4) (k 5 (k 4) (k 4)))))" "(check (Refl (Fapp (Apply (Apply (Con 5) (Con 4)) (Apply (Apply (Con 5) (Con 4)) (Con 4))) (Frule 4 (Su Ze)) (Frule 5 (Pl (Reccall 0) (Reccall 1))))) (Eq (Fapp (Apply (Apply (Con 5) (Con 4)) (Apply (Apply (Con 5) (Con 4)) (Con 4))) (Frule 4 (Su Ze)) (Frule 5 (Pl (Reccall 0) (Reccall 1)))) (Su (Su (Su Ze)))))" accept
# BINARY function: user-add(1,1)=2 — check.beta (f fid x y) + table vs checker.gamma Fbundle + inline rules
dia "fun add(1,1)=2" "(data 2 0 0 0) (data 3 1 1 0) (fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) (= (f 10 (k 3 (k 2)) (k 3 (k 2))) (k 3 (k 3 (k 2)))) (refl (f 10 (k 3 (k 2)) (k 3 (k 2))))" "(check (Refl (Fapp (Fbundle (Apply (Con 3) (Con 2)) (Apply (Con 3) (Con 2))) (Frule 2 (Par 0)) (Frule 3 (Apply (Con 3) (Reccall 0))))) (Eq (Fapp (Fbundle (Apply (Con 3) (Con 2)) (Apply (Con 3) (Con 2))) (Frule 2 (Par 0)) (Frule 3 (Apply (Con 3) (Reccall 0)))) (Apply (Con 3) (Apply (Con 3) (Con 2)))))" accept
# COMPOSED user-function across the diamond: user-mult (fid 11) whose recursive rule body
# CALLS user-add (fid 10). check.beta sources add's rules from its table; checker.gamma must
# carry them INLINE, nested inside mult's Frule 3 — a distinct code path in each checker that
# must still agree. mult(1,2)=2 fires the nested add-call; the !=3 control must reject.
MFB="(fun 11 2 (k 2)) (fun 11 3 (f 10 (y 0) (rec 0)))"
GF="(Fapp (Fbundle (Apply (Con 3) (Con 2)) (Apply (Con 3) (Apply (Con 3) (Con 2)))) (Frule 2 (Con 2)) (Frule 3 (Fapp (Fbundle (Par 0) (Reccall 0)) (Frule 2 (Par 0)) (Frule 3 (Apply (Con 3) (Reccall 0))))))"
dia "fun mult(1,2)=2" "(data 2 0 0 0) (data 3 1 1 0) (fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) $MFB (= (f 11 (k 3 (k 2)) (k 3 (k 3 (k 2)))) (k 3 (k 3 (k 2)))) (refl (f 11 (k 3 (k 2)) (k 3 (k 3 (k 2)))))" "(check (Refl $GF) (Eq $GF (Apply (Con 3) (Apply (Con 3) (Con 2)))))" accept
dia "fun mult(1,2)!=3" "(data 2 0 0 0) (data 3 1 1 0) (fun 10 2 (y 0)) (fun 10 3 (k 3 (rec 0))) $MFB (= (f 11 (k 3 (k 2)) (k 3 (k 3 (k 2)))) (k 3 (k 3 (k 3 (k 2))))) (refl (f 11 (k 3 (k 2)) (k 3 (k 3 (k 2)))))" "(check (Refl $GF) (Eq $GF (Apply (Con 3) (Apply (Con 3) (Apply (Con 3) (Con 2))))))" reject
# the ¬∃ ↔ ∀¬ duality (both directions constructive); ¬∀ -> ∃¬ is NOT — must reject
# the eigenvariable fix, cross-checked: exists-ELIM under an open hypothesis ACCEPTS in both
# checkers, while generalizing a constrained variable (P(x) -> forall y.P(y)) REJECTS in both.
dia "open-hyp unpack" "(All (-> (Exists (= (p (v 1) (v 0)) (v 1))) (Exists (= (p (v 1) (v 0)) (v 1))))) (gen (lam (Exists (= (p (v 1) (v 0)) (v 1))) (unpack (hyp 0) (gen (lam (= (p (v 1) (v 0)) (v 1)) (wit (= (p (v 2) (v 0)) (v 2)) (v 0) (hyp 0)))))))" "(check (Gen (Lam (Exists (Eq (Pl (Iv 1) (Iv 0)) (Iv 1))) (Unpack (Hyp 0) (Gen (Lam (Eq (Pl (Iv 1) (Iv 0)) (Iv 1)) (Wit (Eq (Pl (Iv 2) (Iv 0)) (Iv 2)) (Iv 0) (Hyp 0))))))) (All (Arrow (Exists (Eq (Pl (Iv 1) (Iv 0)) (Iv 1))) (Exists (Eq (Pl (Iv 1) (Iv 0)) (Iv 1))))))" accept
dia "gen no capture"  "(-> (Pred 0 (v 0)) (All (Pred 0 (v 0)))) (lam (Pred 0 (v 0)) (gen (hyp 0)))" "(check (Lam (Pred 0 (Iv 0)) (Gen (Hyp 0))) (Arrow (Pred 0 (Iv 0)) (All (Pred 0 (Iv 0)))))" reject
dia "neg-ex->all-neg" "(-> (-> (Exists (Pred 0 (v 0))) (bot)) (All (-> (Pred 0 (v 0)) (bot)))) (lam (-> (Exists (Pred 0 (v 0))) (bot)) (gen (lam (Pred 0 (v 0)) (app (hyp 1) (wit (Pred 0 (v 0)) (v 0) (hyp 0))))))" "(check (Lam (Arrow (Exists (Pred 0 (Iv 0))) Bot) (Gen (Lam (Pred 0 (Iv 0)) (App (Hyp 1) (Wit (Pred 0 (Iv 0)) (Iv 0) (Hyp 0)))))) (Arrow (Arrow (Exists (Pred 0 (Iv 0))) Bot) (All (Arrow (Pred 0 (Iv 0)) Bot))))" accept
dia "all-neg->neg-ex" "(-> (All (-> (Pred 0 (v 0)) (bot))) (-> (Exists (Pred 0 (v 0))) (bot))) (lam (All (-> (Pred 0 (v 0)) (bot))) (lam (Exists (Pred 0 (v 0))) (unpack (hyp 0) (gen (lam (Pred 0 (v 0)) (app (inst (hyp 2) (v 0)) (hyp 0)))))))" "(check (Lam (All (Arrow (Pred 0 (Iv 0)) Bot)) (Lam (Exists (Pred 0 (Iv 0))) (Unpack (Hyp 0) (Gen (Lam (Pred 0 (Iv 0)) (App (Inst (Hyp 2) (Iv 0)) (Hyp 0))))))) (Arrow (All (Arrow (Pred 0 (Iv 0)) Bot)) (Arrow (Exists (Pred 0 (Iv 0))) Bot)))" accept
dia "neg-all->ex-neg" "(-> (-> (All (Pred 0 (v 0))) (bot)) (Exists (-> (Pred 0 (v 0)) (bot)))) (lam (-> (All (Pred 0 (v 0))) (bot)) (wit (-> (Pred 0 (v 0)) (bot)) (v 0) (lam (Pred 0 (v 0)) (app (hyp 1) (gen (hyp 0))))))" "(check (Lam (Arrow (All (Pred 0 (Iv 0))) Bot) (Wit (Arrow (Pred 0 (Iv 0)) Bot) (Iv 0) (Lam (Pred 0 (Iv 0)) (App (Hyp 1) (Gen (Hyp 0)))))) (Arrow (Arrow (All (Pred 0 (Iv 0))) Bot) (Exists (Arrow (Pred 0 (Iv 0)) Bot))))" reject
# distribution & case-analysis — both checkers agree
dia "&-over-+ dist"  "(-> (& A (+ B C)) (+ (& A B) (& A C))) (lam (& A (+ B C)) (case (snd (hyp 0)) (lam B (inl (& A C) (pair (fst (hyp 1)) (hyp 0)))) (lam C (inr (& A B) (pair (fst (hyp 1)) (hyp 0))))))" "(check (Lam (And (Atom 0) (Or (Atom 1) (Atom 2))) (Case (Snd (Hyp 0)) (Lam (Atom 1) (Inl (And (Atom 0) (Atom 2)) (Pair (Fst (Hyp 1)) (Hyp 0)))) (Lam (Atom 2) (Inr (And (Atom 0) (Atom 1)) (Pair (Fst (Hyp 1)) (Hyp 0)))))) (Arrow (And (Atom 0) (Or (Atom 1) (Atom 2))) (Or (And (Atom 0) (Atom 1)) (And (Atom 0) (Atom 2)))))" accept
dia "case-curry ->"  "(-> (-> (+ A B) C) (& (-> A C) (-> B C))) (lam (-> (+ A B) C) (pair (lam A (app (hyp 1) (inl B (hyp 0)))) (lam B (app (hyp 1) (inr A (hyp 0))))))" "(check (Lam (Arrow (Or (Atom 0) (Atom 1)) (Atom 2)) (Pair (Lam (Atom 0) (App (Hyp 1) (Inl (Atom 1) (Hyp 0)))) (Lam (Atom 1) (App (Hyp 1) (Inr (Atom 0) (Hyp 0)))))) (Arrow (Arrow (Or (Atom 0) (Atom 1)) (Atom 2)) (And (Arrow (Atom 0) (Atom 2)) (Arrow (Atom 1) (Atom 2)))))" accept
# congruence / Leibniz transport via eqelim — both checkers agree
dia "s congruence"   "(All (All (-> (= (v 1) (v 0)) (= (s (v 1)) (s (v 0)))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (s (v 2)) (s (v 0))) (hyp 0) (refl (s (v 1)))))))" "(check (Gen (Gen (Lam (Eq (Iv 1) (Iv 0)) (Eqelim (Eq (Su (Iv 2)) (Su (Iv 0))) (Hyp 0) (Refl (Su (Iv 1))))))) (All (All (Arrow (Eq (Iv 1) (Iv 0)) (Eq (Su (Iv 1)) (Su (Iv 0)))))))" accept
dia "Leibniz transp" "(All (All (-> (= (v 1) (v 0)) (-> (Pred 0 (v 1)) (Pred 0 (v 0)))))) (gen (gen (lam (= (v 1) (v 0)) (lam (Pred 0 (v 1)) (eqelim (Pred 0 (v 0)) (hyp 1) (hyp 0))))))" "(check (Gen (Gen (Lam (Eq (Iv 1) (Iv 0)) (Lam (Pred 0 (Iv 1)) (Eqelim (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))))) (All (All (Arrow (Eq (Iv 1) (Iv 0)) (Arrow (Pred 0 (Iv 1)) (Pred 0 (Iv 0)))))))" accept
dia "Leibniz Rel right" "(All (All (All (-> (= (v 1) (v 0)) (-> (Rel 0 (v 2) (v 1)) (Rel 0 (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 1) (v 0)) (lam (Rel 0 (v 2) (v 1)) (eqelim (Rel 0 (v 3) (v 0)) (hyp 1) (hyp 0)))))))" "(check (Gen (Gen (Gen (Lam (Eq (Iv 1) (Iv 0)) (Lam (Rel 0 (Iv 2) (Iv 1)) (Eqelim (Rel 0 (Iv 3) (Iv 0)) (Hyp 1) (Hyp 0))))))) (All (All (All (Arrow (Eq (Iv 1) (Iv 0)) (Arrow (Rel 0 (Iv 2) (Iv 1)) (Rel 0 (Iv 2) (Iv 0))))))))" accept
# real first-order reasoning: instantiate at the gen-bound variable (open witness)
dia "forall-distrib" "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 0 (v 0))) (All (Pred 1 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 0 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))" "(check (Lam (All (Arrow (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Lam (All (Pred 0 (Iv 0))) (Gen (App (Inst (Hyp 1) (Iv 0)) (Inst (Hyp 0) (Iv 0)))))) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Arrow (All (Pred 0 (Iv 0))) (All (Pred 1 (Iv 0))))))" accept
dia "false converse" "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 1 (v 0))) (All (Pred 0 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 1 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))" "(check (Lam (All (Arrow (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Lam (All (Pred 1 (Iv 0))) (Gen (App (Inst (Hyp 1) (Iv 0)) (Inst (Hyp 0) (Iv 0)))))) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Pred 1 (Iv 0)))) (Arrow (All (Pred 1 (Iv 0))) (All (Pred 0 (Iv 0))))))" reject
# binary relations
dia "rel tautology"  "(All (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (v 0))))) (gen (gen (lam (Rel 0 (v 1) (v 0)) (hyp 0))))" "(check (Gen (Gen (Lam (Rel 0 (Iv 1) (Iv 0)) (Hyp 0)))) (All (All (Arrow (Rel 0 (Iv 1) (Iv 0)) (Rel 0 (Iv 1) (Iv 0))))))" accept
dia "rel args ordered" "(-> (Rel 0 z (s z)) (Rel 0 (s z) z)) (lam (Rel 0 z (s z)) (hyp 0))" "(check (Lam (Rel 0 Ze (Su Ze)) (Hyp 0)) (Arrow (Rel 0 Ze (Su Ze)) (Rel 0 (Su Ze) Ze)))" reject
# capture-avoiding substitution: instantiate under nested quantifiers, and the
# discriminator (a capturing bug would accept the second) — both checkers must agree
dia "inst nested ∀"  "(-> (All (All (Rel 0 (v 1) (v 0)))) (All (Rel 0 (v 0) (v 0)))) (lam (All (All (Rel 0 (v 1) (v 0)))) (gen (inst (inst (hyp 0) (v 0)) (v 0))))" "(check (Lam (All (All (Rel 0 (Iv 1) (Iv 0)))) (Gen (Inst (Inst (Hyp 0) (Iv 0)) (Iv 0)))) (Arrow (All (All (Rel 0 (Iv 1) (Iv 0)))) (All (Rel 0 (Iv 0) (Iv 0)))))" accept
dia "no capture"     "(-> (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 0) (v 0)))))) (lam (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (gen (inst (hyp 0) (v 0))))" "(check (Lam (All (Arrow (Pred 0 (Iv 0)) (All (Rel 0 (Iv 1) (Iv 0))))) (Gen (Inst (Hyp 0) (Iv 0)))) (Arrow (All (Arrow (Pred 0 (Iv 0)) (All (Rel 0 (Iv 1) (Iv 0))))) (All (Arrow (Pred 0 (Iv 0)) (All (Rel 0 (Iv 0) (Iv 0)))))))" reject
# Peano induction, and the soundness discriminator (identity step) — both must agree
dia "induction princ" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" "(check (Lam (Pred 0 Ze) (Lam (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Su (Iv 0))))) (Natind (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))) (Arrow (Pred 0 Ze) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Su (Iv 0))))) (All (Pred 0 (Iv 0))))))" accept
dia "identity step"  "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" "(check (Lam (Pred 0 Ze) (Lam (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0)))) (Natind (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))) (Arrow (Pred 0 Ze) (Arrow (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0)))) (All (Pred 0 (Iv 0))))))" reject
# the capstone: ∀n.(n+0 = n) proved by induction + Leibniz eqelim — both checkers agree
dia "n+0=n by induct" "(All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))" "(check (Natind (Eq (Pl (Iv 0) Ze) (Iv 0)) (Refl Ze) (Gen (Lam (Eq (Pl (Iv 0) Ze) (Iv 0)) (Eqelim (Eq (Su (Pl (Iv 1) Ze)) (Su (Iv 0))) (Hyp 0) (Refl (Su (Pl (Iv 0) Ze))))))) (All (Eq (Pl (Iv 0) Ze) (Iv 0))))" accept
dia "n*0=0 by induct" "(All (= (m (v 0) z) z)) (natind (= (m (v 0) z) z) (refl z) (gen (lam (= (m (v 0) z) z) (hyp 0))))" "(check (Natind (Eq (Mu (Iv 0) Ze) Ze) (Refl Ze) (Gen (Lam (Eq (Mu (Iv 0) Ze) Ze) (Hyp 0)))) (All (Eq (Mu (Iv 0) Ze) Ze)))" accept
dia "0 != 1 (disj)"  "(-> (= z (s z)) (bot)) (lam (= z (s z)) (disj (hyp 0)))" "(check (Lam (Eq Ze (Su Ze)) (Disj (Hyp 0))) (Arrow (Eq Ze (Su Ze)) Bot))" accept
dia "succ injective" "(-> (= (s (v 0)) (s z)) (= (v 0) z)) (lam (= (s (v 0)) (s z)) (sinj (hyp 0)))" "(check (Lam (Eq (Su (Iv 0)) (Su Ze)) (Sinj (Hyp 0))) (Arrow (Eq (Su (Iv 0)) (Su Ze)) (Eq (Iv 0) Ze)))" accept
dia "n != s n"       "(All (-> (= (v 0) (s (v 0))) (bot))) (natind (-> (= (v 0) (s (v 0))) (bot)) (lam (= z (s z)) (disj (hyp 0))) (gen (lam (-> (= (v 0) (s (v 0))) (bot)) (lam (= (s (v 0)) (s (s (v 0)))) (app (hyp 1) (sinj (hyp 0)))))))" "(check (Natind (Arrow (Eq (Iv 0) (Su (Iv 0))) Bot) (Lam (Eq Ze (Su Ze)) (Disj (Hyp 0))) (Gen (Lam (Arrow (Eq (Iv 0) (Su (Iv 0))) Bot) (Lam (Eq (Su (Iv 0)) (Su (Su (Iv 0)))) (App (Hyp 1) (Sinj (Hyp 0))))))) (All (Arrow (Eq (Iv 0) (Su (Iv 0))) Bot)))" accept
dia "0 or successor" "(All (+ (= (v 0) z) (Exists (= (v 1) (s (v 0)))))) (natind (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inl (Exists (= z (s (v 0)))) (refl z)) (gen (lam (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inr (= (s (v 0)) z) (wit (= (s (v 1)) (s (v 0))) (v 0) (refl (s (v 0))))))))" "(check (Natind (Or (Eq (Iv 0) Ze) (Exists (Eq (Iv 1) (Su (Iv 0))))) (Inl (Exists (Eq Ze (Su (Iv 0)))) (Refl Ze)) (Gen (Lam (Or (Eq (Iv 0) Ze) (Exists (Eq (Iv 1) (Su (Iv 0))))) (Inr (Eq (Su (Iv 0)) Ze) (Wit (Eq (Su (Iv 1)) (Su (Iv 0))) (Iv 0) (Refl (Su (Iv 0)))))))) (All (Or (Eq (Iv 0) Ze) (Exists (Eq (Iv 1) (Su (Iv 0)))))))" accept
dia "eq symmetric"   "(All (All (-> (= (v 1) (v 0)) (= (v 0) (v 1))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (v 0) (v 2)) (hyp 0) (refl (v 1))))))" "(check (Gen (Gen (Lam (Eq (Iv 1) (Iv 0)) (Eqelim (Eq (Iv 0) (Iv 2)) (Hyp 0) (Refl (Iv 1)))))) (All (All (Arrow (Eq (Iv 1) (Iv 0)) (Eq (Iv 0) (Iv 1))))))" accept
dia "eq transitive"  "(All (All (All (-> (= (v 2) (v 1)) (-> (= (v 1) (v 0)) (= (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 2) (v 1)) (lam (= (v 1) (v 0)) (eqelim (= (v 3) (v 0)) (hyp 0) (hyp 1)))))))" "(check (Gen (Gen (Gen (Lam (Eq (Iv 2) (Iv 1)) (Lam (Eq (Iv 1) (Iv 0)) (Eqelim (Eq (Iv 3) (Iv 0)) (Hyp 0) (Hyp 1))))))) (All (All (All (Arrow (Eq (Iv 2) (Iv 1)) (Arrow (Eq (Iv 1) (Iv 0)) (Eq (Iv 2) (Iv 0))))))))" accept
# Lists: append computes the same way in both checkers
dia "concat lists"   "(= (app (cons z (cons (s z) nil)) (cons (s (s z)) nil)) (cons z (cons (s z) (cons (s (s z)) nil)))) (refl (cons z (cons (s z) (cons (s (s z)) nil))))" "(check (Refl (Lcons Ze (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)))) (Eq (Lapp (Lcons Ze (Lcons (Su Ze) Lnil)) (Lcons (Su (Su Ze)) Lnil)) (Lcons Ze (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)))))" accept
dia "list distinct"  "(= (cons z nil) (cons (s z) nil)) (refl (cons z nil))" "(check (Refl (Lcons Ze Lnil)) (Eq (Lcons Ze Lnil) (Lcons (Su Ze) Lnil)))" reject
# list induction: forall l. l ++ nil = l, and the identity-step soundness discriminator
dia "l ++ nil = l"   "(All (= (app (v 0) nil) (v 0))) (listind (= (app (v 0) nil) (v 0)) (refl nil) (gen (gen (lam (= (app (v 0) nil) (v 0)) (eqelim (= (cons (v 2) (app (v 1) nil)) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (v 0) nil))))))))" "(check (Listind (Eq (Lapp (Iv 0) Lnil) (Iv 0)) (Refl Lnil) (Gen (Gen (Lam (Eq (Lapp (Iv 0) Lnil) (Iv 0)) (Eqelim (Eq (Lcons (Iv 2) (Lapp (Iv 1) Lnil)) (Lcons (Iv 2) (Iv 0))) (Hyp 0) (Refl (Lcons (Iv 1) (Lapp (Iv 0) Lnil)))))))) (All (Eq (Lapp (Iv 0) Lnil) (Iv 0))))" accept
dia "list ident step" "(-> (Pred 0 nil) (-> (All (All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 nil) (lam (All (All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))) (listind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" "(check (Lam (Pred 0 Lnil) (Lam (All (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0))))) (Listind (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))) (Arrow (Pred 0 Lnil) (Arrow (All (All (Arrow (Pred 0 (Iv 0)) (Pred 0 (Iv 0))))) (All (Pred 0 (Iv 0))))))" reject
dia "append assoc"   "(All (All (All (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1))))))) (gen (gen (listind (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1)))) (refl (app (v 1) (v 0))) (gen (gen (lam (= (app (app (v 0) (v 3)) (v 2)) (app (v 0) (app (v 3) (v 2)))) (eqelim (= (cons (v 2) (app (app (v 1) (v 4)) (v 3))) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (app (v 0) (v 3)) (v 2)))))))))))" "(check (Gen (Gen (Listind (Eq (Lapp (Lapp (Iv 0) (Iv 2)) (Iv 1)) (Lapp (Iv 0) (Lapp (Iv 2) (Iv 1)))) (Refl (Lapp (Iv 1) (Iv 0))) (Gen (Gen (Lam (Eq (Lapp (Lapp (Iv 0) (Iv 3)) (Iv 2)) (Lapp (Iv 0) (Lapp (Iv 3) (Iv 2)))) (Eqelim (Eq (Lcons (Iv 2) (Lapp (Lapp (Iv 1) (Iv 4)) (Iv 3))) (Lcons (Iv 2) (Iv 0))) (Hyp 0) (Refl (Lcons (Iv 1) (Lapp (Lapp (Iv 0) (Iv 3)) (Iv 2))))))))))) (All (All (All (Eq (Lapp (Lapp (Iv 0) (Iv 2)) (Iv 1)) (Lapp (Iv 0) (Lapp (Iv 2) (Iv 1))))))))" accept
dia "len(a++b)"     "(All (All (= (len (app (v 0) (v 1))) (p (len (v 0)) (len (v 1)))))) (gen (listind (= (len (app (v 0) (v 1))) (p (len (v 0)) (len (v 1)))) (refl (len (v 0))) (gen (gen (lam (= (len (app (v 0) (v 2))) (p (len (v 0)) (len (v 2)))) (eqelim (= (s (len (app (v 1) (v 3)))) (s (v 0))) (hyp 0) (refl (s (len (app (v 0) (v 2)))))))))))" "(check (Gen (Listind (Eq (Llen (Lapp (Iv 0) (Iv 1))) (Pl (Llen (Iv 0)) (Llen (Iv 1)))) (Refl (Llen (Iv 0))) (Gen (Gen (Lam (Eq (Llen (Lapp (Iv 0) (Iv 2))) (Pl (Llen (Iv 0)) (Llen (Iv 2)))) (Eqelim (Eq (Su (Llen (Lapp (Iv 1) (Iv 3)))) (Su (Iv 0))) (Hyp 0) (Refl (Su (Llen (Lapp (Iv 0) (Iv 2))))))))))) (All (All (Eq (Llen (Lapp (Iv 0) (Iv 1))) (Pl (Llen (Iv 0)) (Llen (Iv 1)))))))" accept
dia "n+sm=s(n+m)"   "(All (All (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))))) (gen (natind (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (refl (s (v 0))) (gen (lam (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (eqelim (= (s (p (v 1) (s (v 2)))) (s (v 0))) (hyp 0) (refl (s (p (v 0) (s (v 1))))))))))" "(check (Gen (Natind (Eq (Pl (Iv 0) (Su (Iv 1))) (Su (Pl (Iv 0) (Iv 1)))) (Refl (Su (Iv 0))) (Gen (Lam (Eq (Pl (Iv 0) (Su (Iv 1))) (Su (Pl (Iv 0) (Iv 1)))) (Eqelim (Eq (Su (Pl (Iv 1) (Su (Iv 2)))) (Su (Iv 0))) (Hyp 0) (Refl (Su (Pl (Iv 0) (Su (Iv 1)))))))))) (All (All (Eq (Pl (Iv 0) (Su (Iv 1))) (Su (Pl (Iv 0) (Iv 1)))))))" accept
dia "+ associative" "(All (All (All (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1))))))) (gen (gen (natind (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (refl (p (v 1) (v 0))) (gen (lam (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (eqelim (= (s (p (p (v 1) (v 3)) (v 2))) (s (v 0))) (hyp 0) (refl (s (p (p (v 0) (v 2)) (v 1))))))))))" "(check (Gen (Gen (Natind (Eq (Pl (Pl (Iv 0) (Iv 2)) (Iv 1)) (Pl (Iv 0) (Pl (Iv 2) (Iv 1)))) (Refl (Pl (Iv 1) (Iv 0))) (Gen (Lam (Eq (Pl (Pl (Iv 0) (Iv 2)) (Iv 1)) (Pl (Iv 0) (Pl (Iv 2) (Iv 1)))) (Eqelim (Eq (Su (Pl (Pl (Iv 1) (Iv 3)) (Iv 2))) (Su (Iv 0))) (Hyp 0) (Refl (Su (Pl (Pl (Iv 0) (Iv 2)) (Iv 1)))))))))) (All (All (All (Eq (Pl (Pl (Iv 0) (Iv 2)) (Iv 1)) (Pl (Iv 0) (Pl (Iv 2) (Iv 1))))))))" accept
dia "n*1 = n"       "(All (= (m (v 0) (s z)) (v 0))) (natind (= (m (v 0) (s z)) (v 0)) (refl z) (gen (lam (= (m (v 0) (s z)) (v 0)) (eqelim (= (s (m (v 1) (s z))) (s (v 0))) (hyp 0) (refl (s (m (v 0) (s z))))))))" "(check (Natind (Eq (Mu (Iv 0) (Su Ze)) (Iv 0)) (Refl Ze) (Gen (Lam (Eq (Mu (Iv 0) (Su Ze)) (Iv 0)) (Eqelim (Eq (Su (Mu (Iv 1) (Su Ze))) (Su (Iv 0))) (Hyp 0) (Refl (Su (Mu (Iv 0) (Su Ze)))))))) (All (Eq (Mu (Iv 0) (Su Ze)) (Iv 0))))" accept
dia "Node = Node"    "(= (k 1 (k 0) (k 0)) (k 1 (k 0) (k 0))) (refl (k 1 (k 0) (k 0)))" "(check (Refl (Apply (Apply (Con 1) (Con 0)) (Con 0))) (Eq (Apply (Apply (Con 1) (Con 0)) (Con 0)) (Apply (Apply (Con 1) (Con 0)) (Con 0))))" accept
dia "Leaf != Node"   "(= (k 0) (k 1 (k 0) (k 0))) (refl (k 0))" "(check (Refl (Con 0)) (Eq (Con 0) (Apply (Apply (Con 1) (Con 0)) (Con 0))))" reject
dia "Tree induct"   "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" "(check (Lam (Pred 0 (Con 0)) (Lam (All (All (Arrow (Pred 0 (Iv 1)) (Arrow (Pred 0 (Iv 0)) (Pred 0 (Apply (Apply (Con 1) (Iv 1)) (Iv 0))))))) (Rec (Mkspec 0 0 0 0) (Mkspec 1 2 1 1) (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))) (Arrow (Pred 0 (Con 0)) (Arrow (All (All (Arrow (Pred 0 (Iv 1)) (Arrow (Pred 0 (Iv 0)) (Pred 0 (Apply (Apply (Con 1) (Iv 1)) (Iv 0))))))) (All (Pred 0 (Iv 0))))))" accept
dia "rec missing IH" "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" "(check (Lam (Pred 0 (Con 0)) (Lam (All (All (Arrow (Pred 0 (Iv 1)) (Pred 0 (Apply (Apply (Con 1) (Iv 1)) (Iv 0)))))) (Rec (Mkspec 0 0 0 0) (Mkspec 1 2 1 1) (Pred 0 (Iv 0)) (Hyp 1) (Hyp 0)))) (Arrow (Pred 0 (Con 0)) (Arrow (All (All (Arrow (Pred 0 (Iv 1)) (Pred 0 (Apply (Apply (Con 1) (Iv 1)) (Iv 0)))))) (All (Pred 0 (Iv 0))))))" reject
# list membership Mem(x,L) = (Rel 777 x L): inductive predicate, intros + inversions; both agree
dia "mem head"       "(Rel 777 (s (s z)) (cons (s (s z)) nil)) (memhead (s (s z)) nil)" "(check (MemHead (Su (Su Ze)) Lnil) (Rel 777 (Su (Su Ze)) (Lcons (Su (Su Ze)) Lnil)))" accept
dia "mem tail"       "(Rel 777 (s (s z)) (cons (s z) (cons (s (s z)) nil))) (memtail (s z) (memhead (s (s z)) nil))" "(check (MemTail (Su Ze) (MemHead (Su (Su Ze)) Lnil)) (Rel 777 (Su (Su Ze)) (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil))))" accept
dia "mem nil absurd" "(-> (Rel 777 (s z) nil) (bot)) (lam (Rel 777 (s z) nil) (memnil (hyp 0)))" "(check (Lam (Rel 777 (Su Ze) Lnil) (MemNil (Hyp 0))) (Arrow (Rel 777 (Su Ze) Lnil) Bot))" accept
dia "mem cons inv"   "(-> (Rel 777 (s z) (cons (s z) nil)) (+ (= (s z) (s z)) (Rel 777 (s z) nil))) (lam (Rel 777 (s z) (cons (s z) nil)) (memcons (hyp 0)))" "(check (Lam (Rel 777 (Su Ze) (Lcons (Su Ze) Lnil)) (MemCons (Hyp 0))) (Arrow (Rel 777 (Su Ze) (Lcons (Su Ze) Lnil)) (Or (Eq (Su Ze) (Su Ze)) (Rel 777 (Su Ze) Lnil))))" accept
dia "mem false"      "(Rel 777 (s (s z)) nil) (memhead (s (s z)) nil)" "(check (MemHead (Su (Su Ze)) Lnil) (Rel 777 (Su (Su Ze)) Lnil))" reject
dia "prodis nil"     "(Rel 778 nil (s z)) (pnil)" "(check (Pnil) (Rel 778 Lnil (Su Ze)))" accept
dia "prodis cons"    "(Rel 778 (cons (s (s z)) nil) (m (s (s z)) (s z))) (pcons (s (s z)) (pnil))" "(check (Pcons (Su (Su Ze)) (Pnil)) (Rel 778 (Lcons (Su (Su Ze)) Lnil) (Mu (Su (Su Ze)) (Su Ze))))" accept
dia "prodis false"   "(Rel 778 nil (s (s z))) (pnil)" "(check (Pnil) (Rel 778 Lnil (Su (Su Ze))))" reject
dia "prodis nil inv"  "(-> (Rel 778 nil (s (s z))) (= (s (s z)) (s z))) (lam (Rel 778 nil (s (s z))) (prodnilinv (hyp 0)))" "(check (Lam (Rel 778 Lnil (Su (Su Ze))) (Prodnilinv (Hyp 0))) (Arrow (Rel 778 Lnil (Su (Su Ze))) (Eq (Su (Su Ze)) (Su Ze))))" accept
dia "prodis cons inv" "(Exists (& (= (m (s (s z)) (s z)) (m (s (s z)) (v 0))) (Rel 778 nil (v 0)))) (prodconsinv (pcons (s (s z)) (pnil)))" "(check (Prodconsinv (Pcons (Su (Su Ze)) (Pnil))) (Exists (And (Eq (Mu (Su (Su Ze)) (Su Ze)) (Mu (Su (Su Ze)) (Iv 0))) (Rel 778 Lnil (Iv 0)))))" accept
dia "prodis inv false" "(= (m (s (s z)) (s z)) (s z)) (prodnilinv (pcons (s (s z)) (pnil)))" "(check (Prodnilinv (Pcons (Su (Su Ze)) (Pnil))) (Eq (Mu (Su (Su Ze)) (Su Ze)) (Su Ze)))" reject
# inversions UNDER BINDERS: cross-validate check.beta shift_term vs checker.gamma shiftt (deep shift)
dia "pconsinv shift"  "(All (All (All (-> (Rel 778 (cons (v 2) (v 1)) (v 0)) (Exists (& (= (v 1) (m (v 3) (v 0))) (Rel 778 (v 2) (v 0)))))))) (gen (gen (gen (lam (Rel 778 (cons (v 2) (v 1)) (v 0)) (prodconsinv (hyp 0))))))" "(check (Gen (Gen (Gen (Lam (Rel 778 (Lcons (Iv 2) (Iv 1)) (Iv 0)) (Prodconsinv (Hyp 0)))))) (All (All (All (Arrow (Rel 778 (Lcons (Iv 2) (Iv 1)) (Iv 0)) (Exists (And (Eq (Iv 1) (Mu (Iv 3) (Iv 0))) (Rel 778 (Iv 2) (Iv 0)))))))))" accept
dia "memcons binders" "(All (All (-> (Rel 777 (v 1) (cons (v 1) (v 0))) (+ (= (v 1) (v 1)) (Rel 777 (v 1) (v 0)))))) (gen (gen (lam (Rel 777 (v 1) (cons (v 1) (v 0))) (memcons (hyp 0)))))" "(check (Gen (Gen (Lam (Rel 777 (Iv 1) (Lcons (Iv 1) (Iv 0))) (MemCons (Hyp 0))))) (All (All (Arrow (Rel 777 (Iv 1) (Lcons (Iv 1) (Iv 0))) (Or (Eq (Iv 1) (Iv 1)) (Rel 777 (Iv 1) (Iv 0)))))))" accept
dia "pconsinv wrong"  "(All (All (All (-> (Rel 778 (cons (v 2) (v 1)) (v 0)) (Exists (& (= (v 1) (m (v 0) (v 3))) (Rel 778 (v 2) (v 0)))))))) (gen (gen (gen (lam (Rel 778 (cons (v 2) (v 1)) (v 0)) (prodconsinv (hyp 0))))))" "(check (Gen (Gen (Gen (Lam (Rel 778 (Lcons (Iv 2) (Iv 1)) (Iv 0)) (Prodconsinv (Hyp 0)))))) (All (All (All (Arrow (Rel 778 (Lcons (Iv 2) (Iv 1)) (Iv 0)) (Exists (And (Eq (Iv 1) (Mu (Iv 0) (Iv 3))) (Rel 778 (Iv 2) (Iv 0)))))))))" reject
# list permutation Perm(L1,L2) = Rel 779: the four intro rules cross-checked in all three checkers
dia "permnil"         "(Rel 779 nil nil) (permnil)" "(check (Permnil) (Rel 779 Lnil Lnil))" accept
dia "permswap"        "(Rel 779 (cons (s z) (cons (s (s z)) nil)) (cons (s (s z)) (cons (s z) nil))) (permswap (s z) (s (s z)) nil)" "(check (Permswap (Su Ze) (Su (Su Ze)) Lnil) (Rel 779 (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)) (Lcons (Su (Su Ze)) (Lcons (Su Ze) Lnil))))" accept
dia "permskip"        "(Rel 779 (cons (s z) nil) (cons (s z) nil)) (permskip (s z) (permnil))" "(check (Permskip (Su Ze) (Permnil)) (Rel 779 (Lcons (Su Ze) Lnil) (Lcons (Su Ze) Lnil)))" accept
dia "permtrans"       "(Rel 779 (cons (s z) (cons (s (s z)) nil)) (cons (s z) (cons (s (s z)) nil))) (permtrans (permswap (s z) (s (s z)) nil) (permswap (s (s z)) (s z) nil))" "(check (Permtrans (Permswap (Su Ze) (Su (Su Ze)) Lnil) (Permswap (Su (Su Ze)) (Su Ze) Lnil)) (Rel 779 (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil)) (Lcons (Su Ze) (Lcons (Su (Su Ze)) Lnil))))" accept
dia "permnil bogus"   "(Rel 779 (cons (s z) nil) (cons (s (s z)) nil)) (permnil)" "(check (Permnil) (Rel 779 (Lcons (Su Ze) Lnil) (Lcons (Su (Su Ze)) Lnil)))" reject
dia "permtrans mismatch" "(Rel 779 (cons (s z) nil) (cons (s (s z)) nil)) (permtrans (permskip (s z) (permnil)) (permswap (s z) (s (s z)) nil))" "(check (Permtrans (Permskip (Su Ze) (Permnil)) (Permswap (Su Ze) (Su (Su Ze)) Lnil)) (Rel 779 (Lcons (Su Ze) Lnil) (Lcons (Su (Su Ze)) Lnil)))" reject
echo "checker diamond (check.beta vs checker.gamma): $PASS agree, $FAIL disagree"
[ "$FAIL" = 0 ] || exit 1
if [ "$HAVE_TYPED" = 1 ]; then
  echo "  + 3rd oracle (type-erased checker_typed.gamma vs checker.gamma): $TPASS agree, $TFAIL disagree"
  [ "$TFAIL" = 0 ] || exit 1
else
  echo "  + 3rd oracle (typed checker) skipped — python3 absent"
fi
