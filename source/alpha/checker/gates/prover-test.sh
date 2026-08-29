#!/usr/bin/env sh
# PROOF-AUTOMATION FRONT LINE -- the Omega pattern (automation discharges, the kernel checks). The
# untrusted prover (tools/prover.py) searches for a proof of an intuitionistic {-> , &} propositional goal and
# emits a certificate; the trusted kernel (implementations/beta/check.beta, alpha-rooted) must ACCEPT it. This is the
# "authority in the kernel, cleverness on the untrusted side" split: the prover is sound by construction,
# but the kernel -- not the prover -- is what we trust, so EVERY certificate it emits is re-checked.
#   - curated tautologies: the prover finds a proof implementations/beta/check.beta accepts;
#   - non-tautologies: the prover correctly emits no proof (it never fabricates one);
#   - a random fuzz: for every goal the prover proves, implementations/beta/check.beta accepts the cert (broad soundness).
# Needs python3.
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
command -v python3 >/dev/null 2>&1 || { echo "prover-test: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_proof_checker "$T/check.exe" >/dev/null || { echo "checker artifact unavailable"; exit 1; }
CHECK="$T/check.exe"

PASS=0; FAIL=0
ok() {  # a tautology the prover must prove AND the kernel must accept
  cert=$(python3 tools/prover.py "$1")
  if [ "$cert" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : prover found no proof"; return; fi
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : kernel rejected the prover's cert [$v]"; fi
}
no() {  # NOT a tautology: the prover must emit no proof (never fabricate authority)
  cert=$(python3 tools/prover.py "$1")
  if [ "$cert" = unprovable ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $1 : prover fabricated a proof of a non-tautology: $cert"; fi
}
libblock_ok() {  # bank a BIG derived lemma as a shared def/use library BLOCK (emit_lib_block) rather than
  # inlining it -- for a proof that reuses a lemma hundreds of times, inlining explodes past the checker's
  # working set, but the def/use block stays small. The block + a citation of the lemma must kernel-accept.
  python3 tools/prover.py --libblock "$1" 0 > "$T/lb.out"
  if [ "$(cat "$T/lb.out")" = unprovable ]; then FAIL=$((FAIL+1)); echo "  FAIL libblock $1 : unprovable"; return; fi
  cut -f1 "$T/lb.out" | tr -d '\n' > "$T/lb.block"          # tr strips the trailing newline `cut` adds
  lid=$(cut -f2 "$T/lb.out")
  prop=$(python3 tools/prover.py --inline "$1" | cut -f1)          # the DESUGARED goal prop (implementations/beta/check.beta has no Lt sugar)
  { cat "$T/lb.block"; printf ' %s (use %s)' "$prop" "$lid"; } > "$T/lb.cert"
  v=$("$CHECK" < "$T/lb.cert")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL libblock $1 : kernel rejected [$v]"; fi
}

# curated {->,&} intuitionistic tautologies (the schemas implementations/beta/check.beta's corpus uses, propositional part)
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
ok "(All (Le z (m (v 0) (v 0))))"                      # 0 <= a*a  (non-negativity for a COMPOUND term: witness = the term itself)
ok "(All (All (Le z (m (v 1) (v 0)))))"               # 0 <= a*b  (the generic 0<=C rule, C any closed term)
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
# MONOTONICITY (the next contract-obligation class: bounds preserved under +/* -- e.g. scaling a loop index).
# Additive monotonicity rides the existing sum-witness machinery; mult-mono uses the new mult-SCALING witness
# source (witness K*c, discharged by the banked right-distributivity: a*c + K*c = (a+K)*c = b*c).
ok "(All (All (All (-> (Le (v 2) (v 1)) (Le (p (v 2) (v 0)) (p (v 1) (v 0)))))))"   # a<=b => a+c <= b+c
ok "(All (All (All (-> (Le (v 2) (v 1)) (Le (p (v 0) (v 2)) (p (v 0) (v 1)))))))"   # a<=b => c+a <= c+b
# TWO-bound additive monotonicity (add_le_add): combine TWO order facts under +. The sum-witness gains an
# additive two-bound source in BOTH directions -- UPPER (the sum is the goal's LHS: a<=b & c<=d => a+c<=b+d)
# and LOWER (the sum is the goal's RHS, so a CONSTANT lower-bounds it: 1<=a & 1<=b => 2<=a+b). These are the
# range-contract building blocks (`x in lo..=hi` interval bounds compose additively).
ok "(All (All (All (All (-> (Le (v 3) (v 2)) (-> (Le (v 1) (v 0)) (Le (p (v 3) (v 1)) (p (v 2) (v 0)))))))))" # a<=b & c<=d => a+c<=b+d  (two-bound UPPER)
ok "(All (All (-> (Le (s z) (v 1)) (-> (Le (s z) (v 0)) (Le (s (s z)) (p (v 1) (v 0)))))))"                   # 1<=a & 1<=b => 2<=a+b   (two-bound LOWER: constant bounds a sum)
no "(All (All (All (-> (Le (v 2) (v 1)) (Le (p (v 2) (v 0)) (v 1))))))"                                       # a<=b does NOT give a+c<=b (c>0)
ok "(All (All (All (-> (Le (v 2) (v 1)) (Le (m (v 2) (v 0)) (m (v 1) (v 0)))))))"   # a<=b => a*c <= b*c  (mult-mono)
no "(All (All (All (-> (Le (m (v 2) (v 0)) (m (v 1) (v 0))) (Le (v 2) (v 1))))))"   # a*c<=b*c does NOT give a<=b (c=0)
# CANCELLATION (the INVERSE of additive monotonicity). SOUND for + (unlike *, which has the c=0 divisor above):
# from a+c=b+c the induction-on-c + sinj machinery peels the common addend to recover a=b -- no new rule, the
# existing natind/sinj/sum-witness pieces compose. The strict-order sibling a<b => a+c<b+c rides the sum-witness
# source directly. (The <=-cancellation a+c<=b+c => a<=b is NOT yet reached: its natind step needs to compose
# succ-<=-cancel below with the induction hypothesis, which the search doesn't connect within budget -- a known,
# documented gap, not a soundness hole.)
ok "(All (All (All (-> (= (p (v 2) (v 0)) (p (v 1) (v 0))) (= (v 2) (v 1))))))"   # a+c=b+c => a=b  (add-cancel-right)
ok "(All (All (All (-> (= (p (v 0) (v 2)) (p (v 0) (v 1))) (= (v 2) (v 1))))))"   # c+a=c+b => a=b  (add-cancel-left)
ok "(All (All (All (-> (Lt (v 2) (v 1)) (Lt (p (v 2) (v 0)) (p (v 1) (v 0)))))))" # a<b => a+c<b+c  (add-strict-mono)
ok "(All (All (-> (Le (s (v 1)) (s (v 0))) (Le (v 1) (v 0)))))"                   # s a<=s b => a<=b (succ-<=-cancel: le-cancel building block)
no "(All (All (All (All (-> (= (p (v 3) (v 1)) (p (v 2) (v 0))) (= (v 3) (v 2)))))))"  # a+c=b+d does NOT give a=b (unequal addends)
# ORDER + POSITIVITY — fundamental Peano facts (naturals are non-negative, strict order is irreflexive, and 0
# is additively indecomposable). These fall out of the desugared existentials (a<b := ∃k.a+(sk)=b) + disj/sinj:
# a<a would need ∃k.a+(sk)=a (impossible, the successor can't vanish); a+b=0 forces both summands to 0.
ok "(All (-> (Lt (v 0) (v 0)) (bot)))"                          # a < a -> bot   (strict order IRREFLEXIVE)
ok "(All (-> (Lt (v 0) z) (bot)))"                             # a < 0 -> bot   (naturals non-negative; via induction on a)
ok "(All (-> (Le (v 0) z) (= (v 0) z)))"                       # a <= 0 -> a=0  (0 is the order-minimum)
ok "(All (All (-> (= (p (v 1) (v 0)) z) (= (v 1) z))))"        # a+b=0 -> a=0   (additive positivity, left)
ok "(All (All (-> (= (p (v 1) (v 0)) z) (= (v 0) z))))"        # a+b=0 -> b=0   (additive positivity, right)
ok "(All (-> (= (s (v 0)) z) (bot)))"                          # s a = 0 -> bot (Peano's 3rd axiom, explicit)
ok "(All (Lt z (s (v 0))))"                                   # 0 < s a        (every successor is positive)
ok "(All (-> (Lt (s (v 0)) (v 0)) (bot)))"                    # s a < a -> bot (nothing is below its predecessor)
ok "(All (All (-> (= (p (v 1) (s (v 0))) (v 1)) (bot))))"     # a + (s m) = a -> bot (adding a successor is never a no-op; the un-quantified a<a)
ok "(All (All (-> (= (p (v 1) (v 0)) (v 1)) (= (v 0) z))))"   # a + m = a -> m = 0  (CANCEL0: additive cancellation to zero -- needs natind-first on the CANCEL0-shape, see _cancel0_shape)
ok "(All (All (-> (Le (v 1) (v 0)) (Le (s (v 1)) (s (v 0))))))" # a<=b -> s a <= s b (successor MONOTONE; forward dual of succ-<=-cancel)
# STRICT-ORDER ASYMMETRY (a<b & b<a -> bot) -- the first FORWARD order reasoning: a strict cycle in context is
# refuted by chaining it into (Lt A A) (sum-witness) and applying the banked irreflexivity. See the order-cycle
# rule in tools/prover.py. The goal-directed rules alone can't reach it (combining two order facts is forward).
ok "(All (All (-> (Lt (v 1) (v 0)) (-> (Lt (v 0) (v 1)) (bot)))))"  # a<b -> b<a -> bot  (strict order ASYMMETRIC)
# ORDER-EQ (Nat.ne_of_lt): a STRICT fact plus an EQUALITY of its endpoints is absurd -- the equality collapses
# a<b into a<a, refuted by the banked irreflexivity (the order-EQ rule in tools/prover.py). Discharges the source
# contract distinct_when_ordered (i<j -> i!=j). A forward combination the goal-directed rules alone can't reach.
ok "(All (All (-> (Lt (v 1) (v 0)) (-> (= (v 1) (v 0)) (bot)))))"   # a<b -> a=b -> bot  (strict order + eq => absurd)
no "(All (All (-> (Lt (v 1) (v 0)) (= (v 1) (v 0)))))"              # a<b does NOT give a=b
no "(All (All (-> (Lt (v 1) (v 0)) (-> (Lt (v 1) (v 0)) (bot)))))"  # a<b -> a<b -> bot is FALSE (no cycle, just a<b)
# ANTISYMMETRY (a<=b & b<=a -> a=b) -- the partial-order axiom. Forward orchestration on the two <= witnesses:
# the additive cycle a+(k+j)=a gives k+j=0 (CANCEL0), hence k=0 (positivity), hence b=a+k=a+0=a. See the
# le-antisymmetry rule in tools/prover.py; needs CANCEL0 (the natind-first fix above) + positivity, both proven.
ok "(All (All (-> (Le (v 1) (v 0)) (-> (Le (v 0) (v 1)) (= (v 1) (v 0))))))"  # a<=b -> b<=a -> a=b  (ANTISYMMETRIC)
# <=-CANCELLATION (a+c<=b+c -> a<=b) -- the last of the three fundamental order theorems. Witness m=k from the
# <= hypothesis; body a+k=b by rearranging (a+k)+c=(a+c)+k=b+c then cancelling c (add-cancel-right). See the
# le-cancel-right rule; a witness source the directed sum-witness can't reach (its fact's RHS is b+c, not b).
ok "(All (All (All (-> (Le (p (v 2) (v 0)) (p (v 1) (v 0))) (Le (v 2) (v 1))))))"  # a+c<=b+c -> a<=b  (CANCELLATION)
no "(All (All (All (-> (Le (v 2) (p (v 1) (v 0))) (Le (v 2) (v 1))))))"  # a<=b+c does NOT give a<=b (a=b+1, c=1)
no "(All (-> (Lt z (v 0)) (bot)))"                            # 0 < a -> bot is FALSE (a=1)
no "(All (-> (Le (v 0) z) (Lt (v 0) z)))"                     # a<=0 does NOT give a<0 (a=0)
# STRICT mult-mono needs a POSITIVITY guard (a<b => a*c<b*c is FALSE at c=0). With 0<c it discharges via the
# strict mult-SCALING witness w = P+K*c (from 0<c giving c=s P, a<b giving b=a+s K), proved by right-distrib:
# a*c + (s(P+K*c)) = a*c + (s K)*c = (a+s K)*c = b*c. A key fix: natind-first no longer hijacks such IMPLICATION
# goals (it's restricted to bare universal EQUATIONS via _is_ueq) -- inducting on a contract's var is doomed.
ok "(All (All (All (-> (Lt z (v 0)) (-> (Lt (v 2) (v 1)) (Lt (m (v 2) (v 0)) (m (v 1) (v 0))))))))"  # 0<c & a<b => a*c<b*c
no "(All (All (All (-> (Lt (v 2) (v 1)) (Lt (m (v 2) (v 0)) (m (v 1) (v 0)))))))"   # a<b => a*c<b*c is FALSE (c=0)
# ADDITIVE TWO-BOUND composition ("sum of bounded is bounded") -- the additive sibling of strict mult-mono, and
# the building block for value-domain / INTERVAL propagation (a computed x+y carries the range 0..A+B). Witness
# w = K+(s J) from the two `<` facts, closed by add-assoc + add-comm. Works with SYMBOLIC bounds (A+B a stuck
# sum) and CONCRETE ones (A+B a numeral that reduces). It proves the A+B bound, not the tightest.
ok "(All (All (All (All (-> (Lt (v 1) (v 3)) (-> (Lt (v 0) (v 2)) (Lt (p (v 1) (v 0)) (p (v 3) (v 2)))))))))"  # x<A & y<B => x+y<A+B
ok "(All (All (-> (Lt (v 1) (s (s z))) (-> (Lt (v 0) (s (s (s z)))) (Lt (p (v 1) (v 0)) (s (s (s (s (s z))))))))))"  # x<2 & y<3 => x+y<5
no "(All (All (-> (Lt (v 1) (s (s z))) (-> (Lt (v 0) (s (s (s z)))) (Lt (p (v 1) (v 0)) (s (s (s z))))))))"  # x<2 & y<3 => x+y<3 is FALSE (1+2=3)
# The additive two-bound proof reuses add-comm 270+ times, so INLINING it explodes to ~235 KB (past the
# checker's working set). Banked instead as a shared def/use library BLOCK (emit_lib_block) it stays ~58 KB and
# kernel-accepts -- the infrastructure to grow the contract library past inline-able lemmas (e.g. toward wiring
# `self.arr[i+j]` interval-propagation discharge). NB implementations/beta/check.beta's arena is 8 MiB+; the blocker was the inline
# DUPLICATION, not raw size.
libblock_ok "(All (All (All (All (-> (Lt (v 1) (v 3)) (-> (Lt (v 0) (v 2)) (Lt (p (v 1) (v 0)) (p (v 3) (v 2)))))))))"  # additive two-bound, banked def/use
# THE BRIDGE -- WIRED: the lattice's real contract compiler (delta-rust/src/discharge.rs) cites a 7-lemma library
# that is now GENERATED BY THIS PROVER (prover-contract-lib.py -> one self-contained `(def i ..)` per lemma at
# stable ids 0..6), replacing the hand-written .proof base. The 7: add-zero-right, add-commutes, le-trans,
# mult-commutes, add-assoc, MULT-ASSOC (above), lt-le-trans (below) -- all kernel-accepted, all SEARCHED not
# hand-written. "automation discharges with zero hand-proving, the kernel checks" (omega_toolchain.md) is LIVE on the
# lattice's OWN contract pipeline (contracts.sh). These curated goals use the Lt/Le sugar; the library uses the
# desugared Exists form discharge.rs pins -- same propositions after parse.
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
python3 tools/prover.py --batch 150 7 > "$T/certs"
while IFS="$(printf '\t')" read -r goal cert; do
  nproved=$((nproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL fuzz $goal : kernel rejected [$v]"; fi
done < "$T/certs"

# FIRST-ORDER fuzz: provable-by-construction quantifier goals (random predicate/term fillings of the gen /
# inst / wit / unpack schemas) -- every emitted certificate must kernel-accept. This is the soundness net
# for the eigenvariable / de Bruijn emission: a slip there surfaces as a REJECT. Two seeds for breadth.
fonproved=0
python3 tools/prover.py --fobatch 150 7 > "$T/focerts"
python3 tools/prover.py --fobatch 150 3 >> "$T/focerts"
while IFS="$(printf '\t')" read -r goal cert; do
  fonproved=$((fonproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL fo-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/focerts"

# ARITHMETIC fuzz: random closed terms over z/s/p/m, asserted equal to their computed numeral. Each is true
# by construction, so the prover discharges it via refl and the kernel MUST accept -- which validates that the
# prover's normal form agrees with implementations/beta/check.beta's `normalize` (a divergence surfaces as a REJECT). Two seeds.
arnproved=0
python3 tools/prover.py --arithbatch 150 7 > "$T/arcerts"
python3 tools/prover.py --arithbatch 150 5 >> "$T/arcerts"
while IFS="$(printf '\t')" read -r goal cert; do
  arnproved=$((arnproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL arith-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/arcerts"

# INEQUALITY fuzz: provable-by-construction contract-discharge bound goals (transitivity, drop-addend,
# weakenings) with random successor-nested fillings. Hardens the directed sum-witness + lemma + eqelim-chain
# emission (the riskiest recent de Bruijn) -- every emitted certificate must kernel-accept. Two seeds.
iqnproved=0
python3 tools/prover.py --ineqbatch 50 7 > "$T/iqcerts"
python3 tools/prover.py --ineqbatch 50 3 >> "$T/iqcerts"
while IFS="$(printf '\t')" read -r goal cert; do
  iqnproved=$((iqnproved+1))
  v=$(printf '%s' "$cert" | "$CHECK")
  if [ "$v" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL ineq-fuzz $goal : kernel rejected [$v]"; fi
done < "$T/iqcerts"

echo "proof-automation front line (prover discharges; implementations/beta/check.beta validates): $PASS ok ($nproved prop-fuzz, $fonproved fo-fuzz, $arnproved arith-fuzz, $iqnproved ineq-fuzz), $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
