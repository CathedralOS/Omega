#!/usr/bin/env sh
# Gate for the Delta-rung certificate checker. Compiles check.beta with bc (the
# self-hosting, Rust-free Beta compiler), then feeds it proof certificates: valid
# ones must `accept`, invalid ones must `reject`. This is the full lattice stack —
# hand-audited seed -> assembler -> bc -> the checker -> a validated proof.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# build check.exe via bc (cold-start bc through the on-ramp, then bc compiles check)
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < check.beta > "$T/check.asm" || { echo "bc(check.beta) failed"; exit 1; }
"$ASM" < "$T/check.asm" > "$T/check.tape" || { echo "assemble failed"; exit 1; }
stamp_seed "$T/check.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1
echo "check tape: $(wc -c < "$T/check.tape" | tr -d ' ') B (compiled by bc)"

PASS=0; FAIL=0
chk() { # description  "goal term"  expect
  out=$(printf '%s' "$2" | "$T/check.exe")
  if [ "$out" = "$3" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $3 got '$out' : $1"; fi
}
chk "identity"        "(-> P P) (lam P (hyp 0))"                                                    accept
chk "wrong goal"      "(-> P Q) (lam P (hyp 0))"                                                    reject
chk "modus ponens"    "(-> (& (-> P Q) P) Q) (lam (& (-> P Q) P) (app (fst (hyp 0)) (snd (hyp 0))))" accept
chk "curry to pair"   "(-> P (-> Q (& P Q))) (lam P (lam Q (pair (hyp 1) (hyp 0))))"                accept
chk "and-elim"        "(-> (& P Q) P) (lam (& P Q) (fst (hyp 0)))"                                  accept
chk "and-commute"     "(-> (& P Q) (& Q P)) (lam (& P Q) (pair (snd (hyp 0)) (fst (hyp 0))))"       accept
chk "type mismatch"   "(-> (& P Q) Q) (lam (& P Q) (fst (hyp 0)))"                                  reject
chk "unbound hyp"     "P (hyp 0)"                                                                   reject
chk "ill-typed app"   "Q (app (lam P (hyp 0)) (lam Q (hyp 0)))"                                     reject
chk "composition"     "(-> (-> P Q) (-> (-> Q R) (-> P R))) (lam (-> P Q) (lam (-> Q R) (lam P (app (hyp 1) (app (hyp 2) (hyp 0))))))" accept
# falsity ⊥ / negation (¬A = A -> ⊥)
chk "ex falso"        "(-> (bot) P) (lam (bot) (absurd P (hyp 0)))"                                                  accept
chk "neg-elim"        "(-> (-> P (bot)) (-> P Q)) (lam (-> P (bot)) (lam P (absurd Q (app (hyp 1) (hyp 0)))))"       accept
chk "contradiction"   "(-> P (-> (-> P (bot)) Q)) (lam P (lam (-> P (bot)) (absurd Q (app (hyp 0) (hyp 1)))))"       accept
chk "no ex falso"     "(-> (bot) P) (lam (bot) (hyp 0))"                                                             reject
chk "absurd non-bot"  "(-> P Q) (lam P (absurd Q (hyp 0)))"                                                          reject
echo "delta check.beta: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
