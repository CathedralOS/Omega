#!/usr/bin/env sh
# The proof kernel, REWRITTEN IN GAMMA (checker.gamma), run on the Gamma reference
# interpreter (interp.beta, itself compiled Rust-free by bc and run on the seed).
# Same proofs as bootstrap/assurance/proof-kernel/test.sh — valid -> 1, invalid -> 0 — but the
# checker is now ~6 functions of ADTs + pattern matching instead of tagged memory.
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
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < interp.beta > "$T/g.asm" || { echo "bc(interp.beta) failed"; exit 1; }
"$ASM" < "$T/g.asm" > "$T/g.tape" || { echo "assemble failed"; exit 1; }
stamp_seed "$T/g.tape" "$SEED" "$T/g.exe" >/dev/null 2>&1
DEFS=$(cat checker.gamma)

PASS=0; FAIL=0
ck() { # description  "(check PROOF GOAL)"  expect
  printf '%s\n%s\n' "$DEFS" "$2" | "$T/g.exe"; got=$?
  if [ "$got" = "$3" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $3 got $got : $1"; fi
}
#   atoms: P=(Atom 0) Q=(Atom 1)
ck "identity P->P"        "(check (Lam (Atom 0) (Hyp 0)) (Arrow (Atom 0) (Atom 0)))" 1
ck "wrong goal P->Q"     "(check (Lam (Atom 0) (Hyp 0)) (Arrow (Atom 0) (Atom 1)))" 0
ck "and-elim (P&Q)->P"   "(check (Lam (And (Atom 0) (Atom 1)) (Fst (Hyp 0))) (Arrow (And (Atom 0) (Atom 1)) (Atom 0)))" 1
ck "mismatch (P&Q)->Q/fst" "(check (Lam (And (Atom 0) (Atom 1)) (Fst (Hyp 0))) (Arrow (And (Atom 0) (Atom 1)) (Atom 1)))" 0
ck "and-commute"         "(check (Lam (And (Atom 0) (Atom 1)) (Pair (Snd (Hyp 0)) (Fst (Hyp 0)))) (Arrow (And (Atom 0) (Atom 1)) (And (Atom 1) (Atom 0))))" 1
ck "modus ponens"        "(check (Lam (And (Arrow (Atom 0) (Atom 1)) (Atom 0)) (App (Fst (Hyp 0)) (Snd (Hyp 0)))) (Arrow (And (Arrow (Atom 0) (Atom 1)) (Atom 0)) (Atom 1)))" 1
ck "unbound hyp"         "(check (Hyp 0) (Atom 0))" 0
ck "ill-typed app"       "(check (App (Lam (Atom 0) (Hyp 0)) (Lam (Atom 1) (Hyp 0))) (Atom 1))" 0
# disjunction +
ck "inl P->(P+Q)"        "(check (Lam (Atom 0) (Inl (Atom 1) (Hyp 0))) (Arrow (Atom 0) (Or (Atom 0) (Atom 1))))" 1
ck "or-commute"          "(check (Lam (Or (Atom 0) (Atom 1)) (Case (Hyp 0) (Lam (Atom 0) (Inr (Atom 1) (Hyp 0))) (Lam (Atom 1) (Inl (Atom 0) (Hyp 0))))) (Arrow (Or (Atom 0) (Atom 1)) (Or (Atom 1) (Atom 0))))" 1
ck "case branches differ" "(check (Lam (Or (Atom 0) (Atom 1)) (Case (Hyp 0) (Lam (Atom 0) (Hyp 0)) (Lam (Atom 1) (Hyp 0)))) (Arrow (Or (Atom 0) (Atom 1)) (Atom 0)))" 0
# falsity / negation
ck "ex falso bot->P"     "(check (Lam Bot (Absurd (Atom 0) (Hyp 0))) (Arrow Bot (Atom 0)))" 1
ck "no ex falso"         "(check (Lam Bot (Hyp 0)) (Arrow Bot (Atom 0)))" 0
# equality + the conversion rule (Peano: Ze Su Pl)
ck "refl 2+2=4"          "(check (Refl (Su (Su (Su (Su Ze))))) (Eq (Pl (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su Ze))))))" 1
ck "reject 2+2=5"        "(check (Refl (Su (Su (Su (Su Ze))))) (Eq (Pl (Su (Su Ze)) (Su (Su Ze))) (Su (Su (Su (Su (Su Ze)))))))" 0
echo "proof-kernel-in-gamma: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
