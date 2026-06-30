#!/usr/bin/env sh
# PROOF-AUTOMATION FRONT LINE -- the Omega pattern (automation discharges, the kernel checks). The
# untrusted prover (prover.py) searches for a proof of an intuitionistic {-> , &} propositional goal and
# emits a certificate; the trusted kernel (check.beta, alpha-rooted) must ACCEPT it. This is the
# "authority in the kernel, cleverness on the untrusted side" split: the prover is sound by construction,
# but the kernel -- not the prover -- is what we trust, so EVERY certificate it emits is re-checked.
#   - curated tautologies: the prover finds a proof check.beta accepts;
#   - non-tautologies: the prover correctly emits no proof (it never fabricates one);
#   - a random fuzz: for every goal the prover proves, check.beta accepts the cert (broad soundness).
# Needs python3.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "prover-test: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < check.beta > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" \
  && stamp_seed "$T/x.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1 || { echo "build check.beta failed"; exit 1; }
CHECK="$T/check.exe"

PASS=0; FAIL=0
ok() {  # a tautology the prover must prove AND the kernel must accept
  cert=$(python3 prover.py "$1")
  if [ "$cert" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : prover found no proof"; return; fi
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : kernel rejected the prover's cert [$v]"; fi
}
no() {  # NOT a tautology: the prover must emit no proof (never fabricate authority)
  cert=$(python3 prover.py "$1")
  if [ "$cert" = unprovable ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : prover fabricated a proof of a non-tautology: $cert"; fi
}

# curated {->,&} intuitionistic tautologies (the schemas check.beta's corpus uses, propositional part)
ok "(-> P P)"
ok "(-> (& P Q) P)"
ok "(-> (& P Q) Q)"
ok "(-> P (-> Q P))"
ok "(-> (& P Q) (& Q P))"
ok "(-> (& (-> P Q) P) Q)"
ok "(-> (-> (& P Q) R) (-> P (-> Q R)))"
ok "(-> (& (-> P Q) (-> Q R)) (-> P R))"
ok "(-> P (-> (-> P Q) Q))"
ok "(-> (& P (& Q R)) (& (& P Q) R))"
# disjunction (inl/inr/case) and falsity (absurd) -- the full intuitionistic propositional fragment
ok "(-> P (+ P Q))"
ok "(-> (+ P Q) (+ Q P))"
ok "(-> (+ P P) P)"
ok "(-> (bot) P)"
ok "(-> (& P (+ Q R)) (+ (& P Q) (& P R)))"
ok "(-> (& (+ P Q) (-> P R)) (+ R Q))"
ok "(-> (& (-> P R) (-> Q R)) (-> (+ P Q) R))"
# first-order: quantifier introduction/elimination (gen / inst / wit / unpack) over predicates
ok "(All (-> (Pred 0 (v 0)) (Pred 0 (v 0))))"           # forall-id   (gen + ->-intro)
ok "(-> (All (Pred 0 (v 0))) (Pred 0 (s z)))"           # forall-elim (inst at a ground term)
ok "(-> (Pred 0 (s z)) (Exists (Pred 0 (v 0))))"        # exists-intro (wit at the supplied term)
ok "(-> (All (Pred 0 (v 0))) (Exists (Pred 0 (v 0))))"  # forall -> exists (inst then wit, witness z)
ok "(All (All (-> (Rel 0 (v 1) (v 0)) (Rel 0 (v 1) (v 0)))))"          # nested gen (de Bruijn order)
# existential elimination (unpack): the eigenvariable opens an existential hypothesis
ok "(-> (Exists (& (Pred 0 (v 0)) (Pred 1 (v 0)))) (Exists (Pred 0 (v 0))))"        # drop a conjunct under exists
ok "(-> (Exists (& (Pred 0 (v 0)) (Pred 1 (v 0)))) (Exists (& (Pred 1 (v 0)) (Pred 0 (v 0)))))"  # exists-commute
ok "(-> (& (Exists (Pred 0 (v 0))) (All (-> (Pred 0 (v 0)) (Pred 1 (v 0))))) (Exists (Pred 1 (v 0))))"  # E.I. of forall
# equality: refl up to the kernel's term conversion (Peano plus `p` / mult `m`), and the conversion axiom
ok "(= (s z) (s z))"                                    # syntactic reflexivity
ok "(= (p (s z) (s z)) (s (s z)))"                      # 1+1=2     (refl up to normalisation)
ok "(= (m (s (s z)) (s (s z))) (s (s (s (s z)))))"      # 2*2=4
ok "(All (= (p z (v 0)) (v 0)))"                        # forall x. 0+x = x   (symbolic: p z b => b)
ok "(-> (Pred 0 (p (s z) (s z))) (Pred 0 (s (s z))))"   # conversion axiom: P(1+1) |- P(2)
ok "(-> (= (p (s z) (s z)) (v 0)) (= (s (s z)) (v 0)))" # conversion inside an equality hypothesis
# equality REWRITING (eqelim / Leibniz transport): sym, trans, congruence, transport -- all one rule
ok "(-> (= (s z) (s (s z))) (= (s (s z)) (s z)))"                          # symmetry
ok "(-> (& (= (s z) (s (s z))) (= (s (s z)) z)) (= (s z) z))"              # transitivity
ok "(-> (= (s z) (s (s z))) (= (s (s z)) (s (s (s z)))))"                  # congruence under s
ok "(-> (& (Pred 0 (s z)) (= (s z) (s (s z)))) (Pred 0 (s (s z))))"        # transport: P(a), a=b |- P(b)
ok "(-> (& (Pred 0 (s (s z))) (= (s z) (s (s z)))) (Pred 0 (s z)))"        # transport, reverse orientation
ok "(-> (& (Rel 0 (s z) z) (= (s z) (s (s z)))) (Rel 0 (s (s z)) z))"      # rewrite one relation argument
ok "(-> (& (= (s z) z) (Pred 0 (p (s z) (s (s z))))) (Pred 0 (p z (s (s z)))))"  # rewrite a subterm of a p-term
# Peano constructor discrimination: successor injectivity (sinj) + zero/successor clash (disj)
ok "(-> (= z (s z)) (bot))"                             # 0 = 1  ->  bot       (disj: zero != succ)
ok "(-> (= z (s z)) P)"                                 # a clashing hypothesis proves anything (ex falso)
ok "(-> (= (s (s z)) (s (v 0))) (= (s z) (v 0)))"       # s(s 0) = s x  ->  s 0 = x   (sinj)
ok "(-> (= (s (v 0)) (s (v 1))) (= (v 0) (v 1)))"       # s x = s y  ->  x = y        (sinj, free vars)
ok "(-> (= (s (s z)) (s (s (s z)))) (bot))"             # 2 = 3  ->  bot   (sinj; sinj; disj chain)
ok "(-> (= (s z) (s (s z))) (= z (s z)))"               # 1 = 2  ->  0 = 1 (sinj; a TRUE vacuous implication)
# inequality, encoded with no kernel `<`:  a<=b := exists k. a+k=b ;  a<b := exists k. a+(s k)=b
ok "(Lt (s z) (s (s (s z))))"                          # 1 < 3
ok "(Le (s (s z)) (s (s z)))"                          # 2 <= 2
ok "(Lt z (s (s (s (s (s z))))))"                      # 0 < 5
ok "(-> (Lt (v 0) (v 1)) (Le (v 0) (v 1)))"            # weakening x<y |- x<=y (FREE vars under binders)
# free individual variables, closed to eigenvars, emitted under wit/unpack binders (regression for the fix)
ok "(-> (Pred 0 (v 0)) (Exists (Pred 0 (v 0))))"                                  # exists-intro, free witness
ok "(-> (Exists (& (Pred 0 (v 0)) (Rel 0 (v 0) (v 1)))) (Exists (Pred 0 (v 0))))" # unpack with a free var
# arithmetic LEMMAS via Peano induction (natind): base P(0) + step P(n)->P(s n), the step closed by the IH
# after goal-normalisation exposes the reduced successor. The whole multi-step cert is kernel-checked.
ok "(All (= (p (v 0) z) (v 0)))"                                    # forall x. x + 0 = x
ok "(All (All (= (p (v 1) (s (v 0))) (s (p (v 1) (v 0))))))"        # forall x y. x + (s y) = s(x + y)
ok "(All (Le (v 0) (v 0)))"                                         # forall x. x <= x  (symbolic, was blocked)
ok "(All (All (= (p (v 1) (v 0)) (p (v 0) (v 1)))))"               # forall x y. x + y = y + x (commutativity)
ok "(All (Le (v 0) (s (v 0))))"                                    # forall x. x <= s x  (the "x < x+1" bound)
ok "(All (Lt (v 0) (s (v 0))))"                                    # forall x. x < s x   (strict, via induction)
# the def/use LEMMA LIBRARY: arithmetic lemmas (add-0, add-succ, add-comm) proved once, emitted as a (def N)
# prelude, and REUSED in the goal via directed matching -- discharges goals inline induction can't reach.
ok "(All (All (Le (v 0) (p (v 1) (v 0)))))"                        # forall x y. y <= x + y    (needs add-comm)
ok "(All (All (= (p (v 0) (v 1)) (p (v 1) (v 0)))))"               # forall x y. y + x = x + y  (lemma reuse)
ok "(All (All (Le (p (v 1) (v 0)) (p (v 0) (v 1)))))"             # forall x y. x + y <= y + x
# more induction reach: multiplication-by-zero (both sides) and 3-variable associativity, all via natind
ok "(All (= (m (v 0) z) z))"                                       # forall x. x * 0 = 0
ok "(All (= (m z (v 0)) z))"                                       # forall x. 0 * x = 0
ok "(All (All (All (= (p (p (v 2) (v 1)) (v 0)) (p (v 2) (p (v 1) (v 0)))))))"  # (x+y)+z = x+(y+z)  associativity
ok "(All (All (= (m (v 1) (v 0)) (m (v 0) (v 1)))))"               # forall x y. x * y = y * x   (mult-commutes)
ok "(All (All (All (= (m (p (v 2) (v 1)) (v 0)) (p (m (v 2) (v 0)) (m (v 1) (v 0)))))))"  # (x+y)*a = x*a + y*a  (right-distributivity)
ok "(All (All (All (= (m (v 2) (p (v 1) (v 0))) (p (m (v 2) (v 1)) (m (v 2) (v 0)))))))"  # a*(x+y) = a*x + a*y  (LEFT-distributivity)
ok "(All (All (All (= (m (m (v 2) (v 1)) (v 0)) (m (v 2) (m (v 1) (v 0)))))))"  # (a*b)*c = a*(b*c)  MULT-ASSOC (discharge.rs id 28)
# mult-assoc (the 7th/last banked contract lemma) is now in reach: NATIND-FIRST on the multiplicand (gen would
# freeze it atop a stuck mult and explode), then its step rewrites via right-distributivity (banked into the
# library, built standalone-then-with-deps) and the INDUCTION HYPOTHESIS (a local universal equation now usable
# as a directed rewrite). left-distributivity unlocks the same way. The cert (~15 KB) is kernel-accepted.
# CONTRACT-DISCHARGE bound shapes (the realistic array-index / loop obligations, i=v0 len=v1)
ok "(-> (Lt (v 0) (v 1)) (Le (s (v 0)) (v 1)))"        # i<len  =>  i+1<=len   (the core array-bounds step)
ok "(-> (Le (s (v 0)) (v 1)) (Lt (v 0) (v 1)))"        # i+1<=len =>  i<len     (its inverse)
ok "(-> (Le (v 0) (v 1)) (Le (v 0) (s (v 1))))"        # i<=len =>  i<=len+1    (widen the bound)
ok "(-> (Lt (v 0) (v 1)) (Lt (v 0) (s (v 1))))"        # i<len  =>  i<len+1
ok "(All (Le z (v 0)))"                                # forall x. 0 <= x       (naturals are non-negative)
no "(-> (Le (v 0) (v 1)) (Lt (v 0) (v 1)))"            # i<=len does NOT give i<len  (i=len)
no "(All (Lt z (v 0)))"                                # 0 < x is FALSE (x=0)
no "(All (Le (v 0) z))"                                # x <= 0 is FALSE
# <=-TRANSITIVITY via the directed sum-chain rule (witness i+j + add-assoc) -- the contract-chaining step
ok "(-> (& (Le (v 0) (v 1)) (Le (v 1) (v 2))) (Le (v 0) (v 2)))"   # a<=b & b<=c  =>  a<=c
no "(-> (Le (v 0) (v 1)) (Le (v 0) (v 2)))"            # a<=b alone does NOT give a<=c (no chain to c)
ok "(-> (Le (p (v 0) (v 1)) (v 2)) (Le (v 0) (v 2)))"  # i+k<=n  =>  i<=n   (DROP-ADDEND: offset array index)
no "(-> (Le (p (v 0) (v 1)) (v 2)) (Le (v 2) (v 0)))"  # i+k<=n does NOT give n<=i
# N-step transitivity: an arbitrary-length bound chain via a path search over the +slack graph
ok "(-> (& (& (Le (v 0) (v 1)) (Le (v 1) (v 2))) (Le (v 2) (v 3))) (Le (v 0) (v 3)))"  # a<=b<=c<=d => a<=d
# STRICT chaining: a < goal whose path's FIRST edge is strict (add-succ-left peels the (s k) goal slot)
ok "(-> (& (Lt (v 0) (v 1)) (Lt (v 1) (v 2))) (Lt (v 0) (v 2)))"   # a<b & b<c   => a<c   (strict transitivity)
ok "(-> (& (Lt (v 0) (v 1)) (Le (v 1) (v 2))) (Lt (v 0) (v 2)))"   # a<b & b<=c  => a<c   (mixed, first strict)
no "(-> (Lt (v 0) (v 1)) (Lt (v 0) (v 2)))"            # a<b alone does NOT give a<c
# THE BRIDGE: the lattice's real contract compiler (epsilon-rs/src/discharge.rs) cites a lemma base banked by
# HAND-WRITTEN .elab proofs (gen-contract-lib.py). The prover now proves ALL 7 banked lemmas AUTOMATICALLY, by
# search -- add-zero-right (id 0), add-commutes (id 5), le-trans (id 9 -- here), mult-commutes (id 20),
# add-assoc (id 21), MULT-ASSOC (id 28 -- above), and lt-le-trans (id 32 -- below) -- all kernel-accepted. So the
# contract lemma base could be GENERATED entirely by automation instead of hand-coded: "automation discharges
# with zero hand-proving" (rungs/omega.md), on the lattice's OWN contract system. The hand .elab base is now
# fully reproducible by the Rust-free proof-search front line.
ok "(All (All (All (-> (Le (v 2) (v 1)) (-> (Le (v 1) (v 0)) (Le (v 2) (v 0)))))))"  # le-trans (discharge.rs id 9)
ok "(All (All (All (-> (Lt (v 2) (v 1)) (-> (Le (v 1) (v 0)) (Lt (v 2) (v 0)))))))"  # lt-le-trans (discharge.rs id 32)
# non-tautologies: provability must fail (soundness of the front line)
no "(Exists (Pred 0 (v 0)))"                            # no witness available -> unprovable
no "(-> (Exists (Pred 0 (v 0))) (Pred 0 (s z)))"        # eigenvariable must NOT escape the unpack
no "(-> (Exists (Pred 0 (v 0))) (All (Pred 0 (v 0))))"  # exists does NOT give forall
no "(= (s z) z)"                                        # 1 != 0    (refl must NOT fire)
no "(= (p (s z) (s z)) (s z))"                          # 1+1 != 1
no "(-> (Pred 0 (s z)) (Pred 0 (s (s z))))"             # P(1) does NOT give P(2)
no "(-> (= (v 0) (s (v 0))) (bot))"                     # x = s x is NOT a literal clash -> needs induction
no "(All (= (p (v 0) z) (s (v 0))))"                    # forall x. x+0 = s x is FALSE: induction's base fails
no "(Lt (s (s (s z))) (s (s z)))"                       # 3 < 2  -> no
no "(Le (s (s (s z))) (s (s z)))"                       # 3 <= 2 -> no
no "(Lt (s (s z)) (s (s z)))"                           # 2 < 2  -> no
no "(-> P Q)"
no "(& P P)"
no "(-> (-> P Q) P)"
no "(-> (-> P Q) Q)"
no "(+ P Q)"
no "(-> (+ P Q) P)"

# random fuzz: for every goal the prover proves, the kernel must accept the certificate. A single
# `--batch` process generates+proves all goals and prints "<goal>\t<cert>" lines for the provable ones,
# so the only per-goal cost is the (unavoidable) kernel check.
nproved=0
python3 prover.py --batch 150 7 > "$T/certs"
while IFS="$(printf '\t')" read -r goal cert; do
  nproved=$((nproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL fuzz $goal : kernel rejected [$v]"; fi
done < "$T/certs"

# FIRST-ORDER fuzz: provable-by-construction quantifier goals (random predicate/term fillings of the gen /
# inst / wit / unpack schemas) -- every emitted certificate must kernel-accept. This is the soundness net
# for the eigenvariable / de Bruijn emission: a slip there surfaces as a REJECT. Two seeds for breadth.
fonproved=0
python3 prover.py --fobatch 150 7 > "$T/focerts"
python3 prover.py --fobatch 150 3 >> "$T/focerts"
while IFS="$(printf '\t')" read -r goal cert; do
  fonproved=$((fonproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL fo-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/focerts"

# ARITHMETIC fuzz: random closed terms over z/s/p/m, asserted equal to their computed numeral. Each is true
# by construction, so the prover discharges it via refl and the kernel MUST accept -- which validates that the
# prover's normal form agrees with check.beta's `normalize` (a divergence surfaces as a REJECT). Two seeds.
arnproved=0
python3 prover.py --arithbatch 150 7 > "$T/arcerts"
python3 prover.py --arithbatch 150 5 >> "$T/arcerts"
while IFS="$(printf '\t')" read -r goal cert; do
  arnproved=$((arnproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL arith-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/arcerts"

# INEQUALITY fuzz: provable-by-construction contract-discharge bound goals (transitivity, drop-addend,
# weakenings) with random successor-nested fillings. Hardens the directed sum-witness + lemma + eqelim-chain
# emission (the riskiest recent de Bruijn) -- every emitted certificate must kernel-accept. Two seeds.
iqnproved=0
python3 prover.py --ineqbatch 50 7 > "$T/iqcerts"
python3 prover.py --ineqbatch 50 3 >> "$T/iqcerts"
while IFS="$(printf '\t')" read -r goal cert; do
  iqnproved=$((iqnproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL ineq-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/iqcerts"

echo "proof-automation front line (prover discharges; check.beta validates): $PASS ok ($nproved prop-fuzz, $fonproved fo-fuzz, $arnproved arith-fuzz, $iqnproved ineq-fuzz), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
