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
# inequality, encoded with no kernel `<`:  a<=b := exists k. a+k=b ;  a<b := exists k. a+(s k)=b
ok "(Lt (s z) (s (s (s z))))"                          # 1 < 3
ok "(Le (s (s z)) (s (s z)))"                          # 2 <= 2
ok "(Lt z (s (s (s (s (s z))))))"                      # 0 < 5
ok "(-> (Lt (v 0) (v 1)) (Le (v 0) (v 1)))"            # weakening x<y |- x<=y (FREE vars under binders)
# free individual variables, closed to eigenvars, emitted under wit/unpack binders (regression for the fix)
ok "(-> (Pred 0 (v 0)) (Exists (Pred 0 (v 0))))"                                  # exists-intro, free witness
ok "(-> (Exists (& (Pred 0 (v 0)) (Rel 0 (v 0) (v 1)))) (Exists (Pred 0 (v 0))))" # unpack with a free var
# non-tautologies: provability must fail (soundness of the front line)
no "(Exists (Pred 0 (v 0)))"                            # no witness available -> unprovable
no "(-> (Exists (Pred 0 (v 0))) (Pred 0 (s z)))"        # eigenvariable must NOT escape the unpack
no "(-> (Exists (Pred 0 (v 0))) (All (Pred 0 (v 0))))"  # exists does NOT give forall
no "(= (s z) z)"                                        # 1 != 0    (refl must NOT fire)
no "(= (p (s z) (s z)) (s z))"                          # 1+1 != 1
no "(-> (Pred 0 (s z)) (Pred 0 (s (s z))))"             # P(1) does NOT give P(2)
no "(-> (= (s z) (s (s z))) (= z (s z)))"               # rewriting 1=2 does NOT yield 0=1 (and terminates)
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

echo "proof-automation front line (prover discharges; check.beta validates): $PASS ok ($nproved prop-fuzz, $fonproved fo-fuzz, $arnproved arith-fuzz), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
