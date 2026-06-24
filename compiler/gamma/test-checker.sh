#!/usr/bin/env sh
# The Delta checker, REWRITTEN IN GAMMA (checker.gamma), run on the Gamma reference
# interpreter (interp.beta, itself compiled Rust-free by bc and run on the seed).
# Same proofs as compiler/delta/test.sh — valid -> 1, invalid -> 0 — but the
# checker is now ~6 functions of ADTs + pattern matching instead of tagged memory.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < interp.beta > "$T/g.asm" || { echo "bc(interp.beta) failed"; exit 1; }
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
echo "delta-checker-in-gamma: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
