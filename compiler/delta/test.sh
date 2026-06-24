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

# build a .beta program with bc (cold-start bc through the on-ramp once)
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
buildbc() { # src.beta -> $T/out.exe
  ../beta-lang-rs/build/bc.exe < "$1" > "$T/p.asm" || { echo "bc($1) failed"; exit 1; }
  "$ASM" < "$T/p.asm" > "$T/p.tape" || { echo "assemble $1 failed"; exit 1; }
  stamp_seed "$T/p.tape" "$SEED" "$2" >/dev/null 2>&1
  echo "$1 tape: $(wc -c < "$T/p.tape" | tr -d ' ') B (compiled by bc)"
}
buildbc check.beta "$T/check.exe"

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
# disjunction ∨ (inl / inr / case)
chk "inl"             "(-> P (+ P Q)) (lam P (inl Q (hyp 0)))"                                                       accept
chk "inr"             "(-> Q (+ P Q)) (lam Q (inr P (hyp 0)))"                                                       accept
chk "or-commute"      "(-> (+ P Q) (+ Q P)) (lam (+ P Q) (case (hyp 0) (lam P (inr Q (hyp 0))) (lam Q (inl P (hyp 0)))))" accept
chk "case to common"  "(-> (+ P P) P) (lam (+ P P) (case (hyp 0) (lam P (hyp 0)) (lam P (hyp 0))))"                  accept
chk "branches differ" "(-> (+ P Q) P) (lam (+ P Q) (case (hyp 0) (lam P (hyp 0)) (lam Q (hyp 0))))"                  reject
chk "inl wrong goal"  "(-> P (+ Q Q)) (lam P (inl Q (hyp 0)))"                                                       reject
# equality + the conversion rule (refl discharged by definitional computation)
chk "refl 2+2=4"      "(= (p (s (s z)) (s (s z))) (s (s (s (s z)))))  (refl (s (s (s (s z)))))"                     accept
chk "refl 0+3=3"      "(= (p z (s (s (s z)))) (s (s (s z))))  (refl (s (s (s z))))"                                 accept
chk "reject 1+1=1"    "(= (p (s z) (s z)) (s z))  (refl (s z))"                                                     reject
chk "reject 2+2=5"    "(= (p (s (s z)) (s (s z))) (s (s (s (s (s z))))))  (refl (s (s (s (s z)))))"                 reject
chk "eq is first-class" "(-> (= (p (s z) (s z)) (s (s z))) (= (p (s z) (s z)) (s (s z)))) (lam (= (p (s z) (s z)) (s (s z))) (hyp 0))" accept
chk "refl 2*3=6"     "(= (m (s (s z)) (s (s (s z)))) (s (s (s (s (s (s z)))))))  (refl (s (s (s (s (s (s z)))))))" accept
chk "reject 2*3=5"   "(= (m (s (s z)) (s (s (s z)))) (s (s (s (s (s z))))))  (refl (s (s (s (s (s z))))))"        reject

# eq.beta — definitional equality by fuel-bounded normalization (proof by computation)
buildbc eq.beta "$T/eq.exe"
eqk() { # description  "t1 t2"  expect
  out=$(printf '%s' "$2" | "$T/eq.exe")
  if [ "$out" = "$3" ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL want $3 got '$out' : $1"; fi
}
eqk "2+2 = 4"         "(p (s (s z)) (s (s z)))  (s (s (s (s z))))"        equal
eqk "0+n = n"         "(p z (s (s (s z))))  (s (s (s z)))"                equal
eqk "n+0 = n"         "(p (s (s (s z))) z)  (s (s (s z)))"                equal
eqk "associativity"  "(p (s z) (p (s (s z)) (s z)))  (p (p (s z) (s (s z))) (s z))" equal
eqk "2+2 != 5"        "(p (s (s z)) (s (s z)))  (s (s (s (s (s z)))))"    differ
eqk "1+1 != 1"        "(p (s z) (s z))  (s z)"                            differ
eqk "2*3 = 6"         "(m (s (s z)) (s (s (s z))))  (s (s (s (s (s (s z))))))" equal
eqk "0*5 = 0"         "(m z (s (s (s (s (s z))))))  z"                    equal
eqk "2*2 != 5"        "(m (s (s z)) (s (s z)))  (s (s (s (s (s z)))))"    differ

echo "delta (check.beta + eq.beta): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
