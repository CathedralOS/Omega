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
# first-order universal quantifier (All / Pred / individual var (v k) / gen / inst)
chk "forall x.Px->Px" "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (gen (lam (Pred 0 (v 0)) (hyp 0)))"              accept
chk "inst forall@0"   "(-> (All (Pred 0 (v 0))) (Pred 0 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))"          accept
chk "inst distributes" "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (Pred 0 z) (Pred 1 z))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (Pred 0 z) (app (inst (hyp 1) z) (hyp 0))))" accept
chk "pred arg conv"   "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z)))) (lam (Pred 0 (p (s z) (s z))) (hyp 0))"   accept
chk "P0 not forall"   "(-> (Pred 0 z) (All (Pred 0 (v 0)))) (lam (Pred 0 z) (gen (hyp 0)))"                       reject
chk "capture blocked" "(All (-> (Pred 0 (v 0)) (All (Pred 0 (v 0))))) (gen (lam (Pred 0 (v 0)) (gen (hyp 0))))"   reject
chk "inst gives P0"   "(-> (All (Pred 0 (v 0))) (Pred 1 z)) (lam (All (Pred 0 (v 0))) (inst (hyp 0) z))"          reject
# first-order existential quantifier (Exists / wit (∃-intro) / unpack (∃-elim))
chk "exists-intro"    "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) z (hyp 0)))"   accept
chk "exists-intro@2"  "(-> (Pred 0 (s (s z))) (Exists (Pred 0 (v 0)))) (lam (Pred 0 (s (s z))) (wit (Pred 0 (v 0)) (s (s z)) (hyp 0)))" accept
chk "exists-elim"     "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 0 (v 0)) Q)) Q)) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 0 (v 0)) Q)) (unpack (hyp 1) (hyp 0))))" accept
chk "wit mismatch"    "(-> (Pred 0 z) (Exists (Pred 0 (v 0)))) (lam (Pred 0 z) (wit (Pred 0 (v 0)) (s z) (hyp 0)))" reject
chk "witness leak"    "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (Pred 0 (v 0)))) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (unpack (hyp 1) (hyp 0))))" reject
chk "handler mismatch" "(-> (Exists (Pred 0 (v 0))) (-> (All (-> (Pred 1 (v 0)) Q)) Q)) (lam (Exists (Pred 0 (v 0))) (lam (All (-> (Pred 1 (v 0)) Q)) (unpack (hyp 1) (hyp 0))))" reject
# REAL first-order reasoning: instantiate at the gen-bound variable (open witness)
chk "forall-distrib"  "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 0 (v 0))) (All (Pred 1 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 0 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))" accept
chk "forall over &"   "(-> (All (& (Pred 0 (v 0)) (Pred 1 (v 0)))) (All (Pred 0 (v 0)))) (lam (All (& (Pred 0 (v 0)) (Pred 1 (v 0)))) (gen (fst (inst (hyp 0) (v 0)))))" accept
chk "forall reconstruct" "(-> (All (Pred 0 (v 0))) (All (Pred 0 (v 0)))) (lam (All (Pred 0 (v 0))) (gen (inst (hyp 0) (v 0))))" accept
chk "false converse"  "(-> (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (-> (All (Pred 1 (v 0))) (All (Pred 0 (v 0))))) (lam (All (-> (Pred 0 (v 0)) (Pred 1 (v 0)))) (lam (All (Pred 1 (v 0))) (gen (app (inst (hyp 1) (v 0)) (inst (hyp 0) (v 0))))))" reject
# binary relations (Rel id t1 t2) — ordered args, conversion in each
chk "rel tautology"   "(All (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (v 0))))) (gen (gen (lam (Rel 0 (v 1) (v 0)) (hyp 0))))" accept
chk "rel inst diag"   "(-> (All (Rel 0 (v 0) (v 0))) (Rel 0 z z)) (lam (All (Rel 0 (v 0) (v 0))) (inst (hyp 0) z))" accept
chk "rel arg conv"    "(-> (Rel 0 (p (s z) (s z)) z) (Rel 0 (s (s z)) z)) (lam (Rel 0 (p (s z) (s z)) z) (hyp 0))" accept
chk "rel args ordered" "(-> (Rel 0 z (s z)) (Rel 0 (s z) z)) (lam (Rel 0 z (s z)) (hyp 0))"                       reject
# instantiation UNDER nested quantifiers — needs capture-avoiding substitution (shifting)
chk "inst nested ∀"   "(-> (All (All (Rel 0 (v 1) (v 0)))) (All (Rel 0 (v 0) (v 0)))) (lam (All (All (Rel 0 (v 1) (v 0)))) (gen (inst (inst (hyp 0) (v 0)) (v 0))))" accept
chk "shift correct"   "(-> (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0)))))) (lam (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (gen (inst (hyp 0) (v 0))))" accept
chk "no capture"      "(-> (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 0) (v 0)))))) (lam (All (-> (Pred 0 (v 0)) (All (Rel 0 (v 1) (v 0))))) (gen (inst (hyp 0) (v 0))))" reject
# Peano induction (natind motive base step)
chk "induction princ" "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "induction param" "(All (-> (Rel 0 (v 0) z) (-> (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (s (v 0))))) (All (Rel 0 (v 1) (v 0)))))) (gen (lam (Rel 0 (v 0) z) (lam (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (s (v 0))))) (natind (Rel 0 (v 1) (v 0)) (hyp 1) (hyp 0)))))" accept
chk "induction n=n"   "(All (= (v 0) (v 0))) (natind (= (v 0) (v 0)) (refl z) (gen (lam (= (v 0) (v 0)) (refl (s (v 0))))))" accept
chk "identity step"   "(-> (Pred 0 z) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (All (Pred 0 (v 0))))) (lam (Pred 0 z) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (v 0)))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
chk "wrong base"      "(-> (Pred 0 (s z)) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (s z)) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (s (v 0))))) (natind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
# Leibniz equality-elimination (eqelim) + a real theorem proved by induction
chk "transport"      "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z)))) (lam (Pred 0 (p (s z) (s z))) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" accept
chk "n+0=n induction" "(All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))" accept
chk "n+0=n not refl"  "(All (= (p (v 0) z) (v 0))) (gen (refl (v 0)))"                                            reject
chk "eqelim mismatch" "(-> (Pred 0 (s z)) (Pred 0 (s (s z)))) (lam (Pred 0 (s z)) (eqelim (Pred 0 (v 0)) (refl (s (s z))) (hyp 0)))" reject
chk "n*0=0 induction" "(All (= (m (v 0) z) z)) (natind (= (m (v 0) z) z) (refl z) (gen (lam (= (m (v 0) z) z) (hyp 0))))" accept
chk "0*n=0 definit'l" "(All (= (m z (v 0)) z)) (gen (refl z))"                                                  accept
# Peano no-confusion: disjointness (0 != s n) and injectivity of successor
chk "0 != 1"         "(-> (= z (s z)) (bot)) (lam (= z (s z)) (disj (hyp 0)))"                                  accept
chk "0=2 -> anything" "(-> (= z (s (s z))) P) (lam (= z (s (s z))) (absurd P (disj (hyp 0))))"                  accept
chk "succ injective"  "(-> (= (s (v 0)) (s z)) (= (v 0) z)) (lam (= (s (v 0)) (s z)) (sinj (hyp 0)))"          accept
chk "disj needs s/0"  "(-> (= (s z) (s z)) (bot)) (lam (= (s z) (s z)) (disj (hyp 0)))"                         reject
chk "sinj wrong"      "(-> (= (s z) (s z)) (= z (s z))) (lam (= (s z) (s z)) (sinj (hyp 0)))"                    reject
# flagship: ∀n. n ≠ s n — induction, base by disj, step by sinj + the hypothesis
chk "n != s n"        "(All (-> (= (v 0) (s (v 0))) (bot))) (natind (-> (= (v 0) (s (v 0))) (bot)) (lam (= z (s z)) (disj (hyp 0))) (gen (lam (-> (= (v 0) (s (v 0))) (bot)) (lam (= (s (v 0)) (s (s (v 0)))) (app (hyp 1) (sinj (hyp 0)))))))" accept
# ∃-intro now admits an OPEN witness (capture-avoiding), enabling the cover theorem
chk "exists open wit" "(All (-> (Rel 0 z (v 0)) (Exists (Rel 0 z (v 0))))) (gen (lam (Rel 0 z (v 0)) (wit (Rel 0 z (v 0)) (v 0) (hyp 0))))" accept
chk "0 or successor"  "(All (+ (= (v 0) z) (Exists (= (v 1) (s (v 0)))))) (natind (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inl (Exists (= z (s (v 0)))) (refl z)) (gen (lam (+ (= (v 0) z) (Exists (= (v 1) (s (v 0))))) (inr (= (s (v 0)) z) (wit (= (s (v 1)) (s (v 0))) (v 0) (refl (s (v 0))))))))" accept
# equality is an equivalence relation: symmetry and transitivity via eqelim
chk "eq symmetric"    "(All (All (-> (= (v 1) (v 0)) (= (v 0) (v 1))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (v 0) (v 2)) (hyp 0) (refl (v 1))))))" accept
chk "eq transitive"   "(All (All (All (-> (= (v 2) (v 1)) (-> (= (v 1) (v 0)) (= (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 2) (v 1)) (lam (= (v 1) (v 0)) (eqelim (= (v 3) (v 0)) (hyp 0) (hyp 1)))))))" accept
# Lists — a SECOND inductive type; append computes under the conversion rule
chk "[0]++[] = [0]"   "(= (app (cons z nil) nil) (cons z nil)) (refl (cons z nil))"                              accept
chk "[]++[0] = [0]"   "(= (app nil (cons z nil)) (cons z nil)) (refl (cons z nil))"                              accept
chk "concat 3 lists"  "(= (app (cons z (cons (s z) nil)) (cons (s (s z)) nil)) (cons z (cons (s z) (cons (s (s z)) nil)))) (refl (cons z (cons (s z) (cons (s (s z)) nil))))" accept
chk "append assoc(c)" "(= (app (app (cons z nil) (cons (s z) nil)) (cons (s (s z)) nil)) (app (cons z nil) (app (cons (s z) nil) (cons (s (s z)) nil)))) (refl (cons z (cons (s z) (cons (s (s z)) nil))))" accept
chk "[0] != [1]"      "(= (cons z nil) (cons (s z) nil)) (refl (cons z nil))"                                     reject
# list induction (listind) + a real theorem: forall l. l ++ nil = l
chk "list ind princ"  "(-> (Pred 0 nil) (-> (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 nil) (lam (All (All (-> (Pred 0 (v 0)) (Pred 0 (cons (v 1) (v 0)))))) (listind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "l ++ nil = l"    "(All (= (app (v 0) nil) (v 0))) (listind (= (app (v 0) nil) (v 0)) (refl nil) (gen (gen (lam (= (app (v 0) nil) (v 0)) (eqelim (= (cons (v 2) (app (v 1) nil)) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (v 0) nil))))))))" accept
chk "l++nil not refl" "(All (= (app (v 0) nil) (v 0))) (gen (refl (v 0)))"                                        reject
chk "list ident step" "(-> (Pred 0 nil) (-> (All (All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 nil) (lam (All (All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))) (listind (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
# capstone: append is associative — ∀a b c. (a++b)++c = a++(b++c), induction on a
chk "append assoc"   "(All (All (All (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1))))))) (gen (gen (listind (= (app (app (v 0) (v 2)) (v 1)) (app (v 0) (app (v 2) (v 1)))) (refl (app (v 1) (v 0))) (gen (gen (lam (= (app (app (v 0) (v 3)) (v 2)) (app (v 0) (app (v 3) (v 2)))) (eqelim (= (cons (v 2) (app (app (v 1) (v 4)) (v 3))) (cons (v 2) (v 0))) (hyp 0) (refl (cons (v 1) (app (app (v 0) (v 3)) (v 2)))))))))))" accept
# length connects the two inductive types: len computes, and len(a++b) = len(a)+len(b)
chk "len [_,_,_]=3"  "(= (len (cons z (cons z (cons z nil)))) (s (s (s z)))) (refl (s (s (s z))))"               accept
chk "len(a++b)"     "(All (All (= (len (app (v 0) (v 1))) (p (len (v 0)) (len (v 1)))))) (gen (listind (= (len (app (v 0) (v 1))) (p (len (v 0)) (len (v 1)))) (refl (len (v 0))) (gen (gen (lam (= (len (app (v 0) (v 2))) (p (len (v 0)) (len (v 2)))) (eqelim (= (s (len (app (v 1) (v 3)))) (s (v 0))) (hyp 0) (refl (s (len (app (v 0) (v 2)))))))))))" accept
chk "n+1 = s n"     "(All (= (p (v 0) (s z)) (s (v 0)))) (natind (= (p (v 0) (s z)) (s (v 0))) (refl (s z)) (gen (lam (= (p (v 0) (s z)) (s (v 0))) (eqelim (= (s (p (v 1) (s z))) (s (v 0))) (hyp 0) (refl (s (p (v 0) (s z))))))))" accept
chk "n+sm=s(n+m)"   "(All (All (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))))) (gen (natind (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (refl (s (v 0))) (gen (lam (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (eqelim (= (s (p (v 1) (s (v 2)))) (s (v 0))) (hyp 0) (refl (s (p (v 0) (s (v 1))))))))))" accept
chk "+ associative"  "(All (All (All (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1))))))) (gen (gen (natind (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (refl (p (v 1) (v 0))) (gen (lam (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (eqelim (= (s (p (p (v 1) (v 3)) (v 2))) (s (v 0))) (hyp 0) (refl (s (p (p (v 0) (v 2)) (v 1))))))))))" accept
# multiplication identities: n*1 = n (induction) and 1*n = n (cites n+0=n via the lemma layer)
chk "n*1 = n"       "(All (= (m (v 0) (s z)) (v 0))) (natind (= (m (v 0) (s z)) (v 0)) (refl z) (gen (lam (= (m (v 0) (s z)) (v 0)) (eqelim (= (s (m (v 1) (s z))) (s (v 0))) (hyp 0) (refl (s (m (v 0) (s z))))))))" accept
chk "1*n = n"       "(def 0 (All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))) (All (= (m (s z) (v 0)) (v 0))) (gen (inst (use 0) (v 0)))" accept
# named lemmas: (def N type proof) verified up front, then (use N) cites it
chk "lemma define/cite" "(def 0 (-> P P) (lam P (hyp 0))) (-> P P) (use 0)"                                       accept
chk "lemma must check" "(def 0 (-> P Q) (lam P (hyp 0))) (-> P Q) (use 0)"                                        reject
chk "cite must match"  "(def 0 (-> P P) (lam P (hyp 0))) (-> Q Q) (use 0)"                                        reject
chk "lemma cites lemma" "(def 0 (-> P P) (lam P (hyp 0))) (def 1 (-> (-> P P) (-> P P)) (lam (-> P P) (use 0))) (-> (-> P P) (-> P P)) (use 1)" accept
# multi-lemma composition: (a+0)+0 = a  via  n+0=n (cited twice) + transitivity
chk "(a+0)+0 = a"   "(def 0 (All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z))))))) ) (def 1 (All (All (All (-> (= (v 2) (v 1)) (-> (= (v 1) (v 0)) (= (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 2) (v 1)) (lam (= (v 1) (v 0)) (eqelim (= (v 3) (v 0)) (hyp 0) (hyp 1)))))))) (All (= (p (p (v 0) z) z) (v 0))) (gen (app (app (inst (inst (inst (use 1) (p (p (v 0) z) z)) (p (v 0) z)) (v 0)) (inst (use 0) (p (v 0) z))) (inst (use 0) (v 0))))" accept

chk "+ commutative"  "(def 0 (All (= (p (v 0) z) (v 0))) (natind (= (p (v 0) z) (v 0)) (refl z) (gen (lam (= (p (v 0) z) (v 0)) (eqelim (= (s (p (v 1) z)) (s (v 0))) (hyp 0) (refl (s (p (v 0) z)))))))) (def 1 (All (All (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))))) (gen (natind (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (refl (s (v 0))) (gen (lam (= (p (v 0) (s (v 1))) (s (p (v 0) (v 1)))) (eqelim (= (s (p (v 1) (s (v 2)))) (s (v 0))) (hyp 0) (refl (s (p (v 0) (s (v 1))))))))))) (def 2 (All (All (-> (= (v 1) (v 0)) (= (v 0) (v 1))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (v 0) (v 2)) (hyp 0) (refl (v 1))))))) (def 3 (All (All (All (-> (= (v 2) (v 1)) (-> (= (v 1) (v 0)) (= (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 2) (v 1)) (lam (= (v 1) (v 0)) (eqelim (= (v 3) (v 0)) (hyp 0) (hyp 1)))))))) (All (All (= (p (v 0) (v 1)) (p (v 1) (v 0))))) (gen (natind (= (p (v 0) (v 1)) (p (v 1) (v 0))) (eqelim (= (v 0) (p (v 1) z)) (inst (use 0) (v 0)) (refl (p (v 0) z))) (gen (lam (= (p (v 0) (v 1)) (p (v 1) (v 0))) (app (app (inst (inst (inst (use 3) (s (p (v 0) (v 1)))) (s (p (v 1) (v 0)))) (p (v 1) (s (v 0)))) (eqelim (= (s (p (v 1) (v 2))) (s (v 0))) (hyp 0) (refl (s (p (v 0) (v 1)))))) (app (inst (inst (use 2) (p (v 1) (s (v 0)))) (s (p (v 1) (v 0)))) (inst (inst (use 1) (v 0)) (v 1))))))))" accept
chk "right distrib"  "(def 0 (All (All (All (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1))))))) (gen (gen (natind (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (refl (p (v 1) (v 0))) (gen (lam (= (p (p (v 0) (v 2)) (v 1)) (p (v 0) (p (v 2) (v 1)))) (eqelim (= (s (p (p (v 1) (v 3)) (v 2))) (s (v 0))) (hyp 0) (refl (s (p (p (v 0) (v 2)) (v 1))))))))))) (def 1 (All (All (-> (= (v 1) (v 0)) (= (v 0) (v 1))))) (gen (gen (lam (= (v 1) (v 0)) (eqelim (= (v 0) (v 2)) (hyp 0) (refl (v 1))))))) (def 2 (All (All (All (-> (= (v 2) (v 1)) (-> (= (v 1) (v 0)) (= (v 2) (v 0))))))) (gen (gen (gen (lam (= (v 2) (v 1)) (lam (= (v 1) (v 0)) (eqelim (= (v 3) (v 0)) (hyp 0) (hyp 1)))))))) (All (All (All (= (m (p (v 0) (v 2)) (v 1)) (p (m (v 0) (v 1)) (m (v 2) (v 1))))))) (gen (gen (natind (= (m (p (v 0) (v 2)) (v 1)) (p (m (v 0) (v 1)) (m (v 2) (v 1)))) (refl (m (v 1) (v 0))) (gen (lam (= (m (p (v 0) (v 2)) (v 1)) (p (m (v 0) (v 1)) (m (v 2) (v 1)))) (app (app (inst (inst (inst (use 2) (p (v 1) (m (p (v 0) (v 2)) (v 1)))) (p (v 1) (p (m (v 0) (v 1)) (m (v 2) (v 1))))) (p (p (v 1) (m (v 0) (v 1))) (m (v 2) (v 1)))) (eqelim (= (p (v 2) (m (p (v 1) (v 3)) (v 2))) (p (v 2) (v 0))) (hyp 0) (refl (p (v 1) (m (p (v 0) (v 2)) (v 1)))))) (app (inst (inst (use 1) (p (p (v 1) (m (v 0) (v 1))) (m (v 2) (v 1)))) (p (v 1) (p (m (v 0) (v 1)) (m (v 2) (v 1))))) (inst (inst (inst (use 0) (m (v 0) (v 1))) (m (v 2) (v 1))) (v 1)))))))))" accept
# generic USER constructors (k cid args...): a Tree = Leaf (k 0) | Node (k 1 l r), inert + structural
chk "Leaf = Leaf"    "(= (k 0) (k 0)) (refl (k 0))"                                                             accept
chk "Node = Node"    "(= (k 1 (k 0) (k 0)) (k 1 (k 0) (k 0))) (refl (k 1 (k 0) (k 0)))"                         accept
chk "Leaf != Node"   "(= (k 0) (k 1 (k 0) (k 0))) (refl (k 0))"                                                 reject
chk "subtrees differ" "(= (k 1 (k 0) (k 0)) (k 1 (k 0) (k 1 (k 0) (k 0)))) (refl (k 1 (k 0) (k 0)))"           reject
chk "Node field conv" "(= (k 1 (p (s z) (s z)) (k 0)) (k 1 (s (s z)) (k 0))) (refl (k 1 (s (s z)) (k 0)))"     accept
chk "Node field diff" "(= (k 1 (s z) (k 0)) (k 1 (s (s z)) (k 0))) (refl (k 1 (s z) (k 0)))"                   reject
# GENERAL structural induction (rec) over a user-DECLARED type, from (data cid arity r0 r1)
chk "Tree induction"  "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "user-Nat induct" "(data 2 0 0 0) (data 3 1 1 0) (-> (Pred 0 (k 2)) (-> (All (-> (Pred 0 (v 0)) (Pred 0 (k 3 (v 0))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 2)) (lam (All (-> (Pred 0 (v 0)) (Pred 0 (k 3 (v 0))))) (rec 2 3 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" accept
chk "rec missing IH"  "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 0)) (-> (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 0)) (lam (All (All (-> (Pred 0 (v 1)) (Pred 0 (k 1 (v 1) (v 0)))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
chk "rec wrong base"  "(data 0 0 0 0) (data 1 2 1 1) (-> (Pred 0 (k 1 (k 0) (k 0))) (-> (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (All (Pred 0 (v 0))))) (lam (Pred 0 (k 1 (k 0) (k 0))) (lam (All (All (-> (Pred 0 (v 1)) (-> (Pred 0 (v 0)) (Pred 0 (k 1 (v 1) (v 0))))))) (rec 0 1 (Pred 0 (v 0)) (hyp 1) (hyp 0))))" reject
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
eqk "[0]++[1]=[0,1]"  "(app (cons z nil) (cons (s z) nil))  (cons z (cons (s z) nil))" equal
eqk "[]++[0]=[0]"     "(app nil (cons z nil))  (cons z nil)"              equal
eqk "len [_,_]=2"     "(len (cons z (cons z nil)))  (s (s z))"            equal
eqk "len(a++b)"       "(len (app (cons z nil) (cons z nil)))  (s (s z))"  equal
eqk "[0] != []"       "(app (cons z nil) nil)  nil"                       differ

echo "delta (check.beta + eq.beta): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
