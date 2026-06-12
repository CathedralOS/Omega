# Proof Engine Roadmap

Status doc for the requires->ensures entailment engine. This maps the theorem
ladder in `canaries/pass/proofs/` and `samples/math_proofs/` to the engine
increments each rung needs, and records what the checker actually discharges
today (2026-06).

## UPDATE 2026-06-12: L7 IS DISCHARGED (induction via recursive contracts + decreases)

The entailment engine now judges INDUCTIVE theorems: single-state machines
whose body is a chain of guarded transitions whose arms are either values or
tail self-calls (the shape the frontend accepts for value recursion --
compound arm expressions like `self.f(n - 1) + n` do not parse, so theorems
are stated in accumulator form). Each arm is one obligation: the ensures with
`result` bound to the arm's value, under requires plus the arm's guard
polarity (the dispatch lowering wraps arm guards as `scrutinee == true` /
`scrutinee == false`; the engine unwraps and folds the polarity into the
comparison).

THE INDUCTION HYPOTHESIS: on a tail self-call arm the engine instantiates the
machine's own ensures over the call's arguments (simultaneous substitution of
parameter atoms by argument polynomials; `result` stays shared because the
arm's result IS the call's result), exactly as a nested callee's ensures
enters a caller context. SOUNDNESS GATE, per call site: the hypothesis enters
only after the engine proves, from that arm's own facts (requires + guard, no
hypothesis), that the declared `decreases` measure is BOTH strictly smaller at
the call's arguments AND still non-negative there -- a strictly decreasing
integer measure bounded below admits no infinite descent, which is the
well-foundedness that makes assuming the contract for the smaller instance
sound. Only the plain descending-naturals reading (`decreases value` /
`-> Nat::Descending`) is verified; view and declared-measure orders never gate
a hypothesis in. No decreases claim, or an undischarged one at that call site,
means no hypothesis -- and rejection is then suppressed (the missing
hypothesis, not the theorem, may be at fault), so the bodied status quo of
accept-by-default is preserved. The machine-level termination pass
independently re-checks the declared clause and fails compilation when it
cannot prove it.

The discharge plumbing: induction hypotheses arrive as general polynomial
equations (e.g. `2*result - P >= 0`) that fit neither the difference-bound
matrix nor the interval evaluator, so `prove_at_least` gained direct
subsumption against stored hypothesis bounds (canonical-form identity). The
base arm grounds `result` through a directed substitution to the arm's value
polynomial and discharges through the existing matrix/interval machinery.

Canaries: `pass/proofs/proof_inductive_gauss_sum` (accumulator Gauss sum,
2*gauss_sum(n, acc) == 2*acc + n*(n+1)) proves by induction;
`fail/proofs/inductive_gauss_sum_false_twin` (off-by-one -- the inductive step
is self-consistent, the BASE arm disproves by constant arithmetic) and
`fail/proofs/inductive_gauss_sum_step_false_twin` (base true, step false --
the recursive arm rejects as unprovable with the hypothesis available) pin
both failure directions. The eight L0-L6 false twins promoted on 2026-06-10
were also finally REGISTERED in the canary suite's fail list (they had been
moved on disk but never wired in; `bag_view_false_twin`'s expected fragment
was stale and now matches the engine's constant-arithmetic diagnostic).

STAND-DOWN preserved: bodies containing anything but the recognized
transition chain (statements, multi-state graphs, non-self transitions,
unreadable arm values/arguments) are not judged at all, and membership facts
or out-of-language conjuncts suppress rejection exactly as before.

## UPDATE 2026-06-10: L0-L6 ARE DISCHARGED

The entailment engine landed in
`compiler/semantics/omega-validation/src/contract_entailment.rs` (superseding
the refutation pass). For an EMPTY-BODY proof machine whose contract lies
inside the engine's language, every ensures fact is now PROVEN or REJECTED --
silent acceptance of a false theorem is over. All 10 `pass/proofs/` rungs
prove; all 8 false twins were promoted from `pending/proofs/` to active
`fail/proofs/` canaries.

Engine shape (see the module doc): canonical polynomials over atoms (constant
folding, congruence, commutativity, distributivity), directed substitutions
from requires equations, a difference-bound matrix with transitive closure
(order transitivity, antisymmetry, unsat detection for vacuity), and a
correlated-power interval evaluator (range sums, squares, the euclidean mod
lemma). Out-of-language contracts (domain membership, unknown calls,
non-parameter places) STAND DOWN: the engine proves what it can but never
rejects what it cannot fully read. `OMEGA_ENTAILMENT_TRACE=1` traces
judgments.

Open from here: the original anchoring gap below still stands for machines
WITH general bodies (their ensures flow through `build_contract_exit_facts`,
which only anchors a trailing expression statement; the L7 update above
covers all-transition bodies separately); the ladder's next rungs are
quantified facts and lowering proof views beyond opaque equality.

## The Empirical Finding

The contract surface is theorem-shaped (Chapter 10: empty-body machines with
`requires`/`ensures` are proof artifacts), but requires->ensures entailment is
NOT discharged. The structural reason, located in
`compiler/pipeline/omega-typed-trees-to-checked-trees/src/proof/contracts/calls.rs`
(`build_contract_exit_facts`): an ensures obligation is only anchored to an
exit when the state's LAST statement is an `Expression` statement. An
empty-body proof machine has no statements, so no `ContractExitFact` is
created and `check_exit_ensures` never runs for it. A false theorem such as

```omega
machine bogus(i: usize, j: usize)
requires
    i < j
ensures
    j < i
{
}
```

passed `--check` until the contract refutation pass landed (see below).

Meanwhile, several specific obligation families ARE enforced today:

- slice bounds ("cannot prove index 15 is within length 8"),
- termination via `terminates { decreases ... }` (`canaries/pass/termination/`),
- caller-side `requires` discharge at call sites (`constraints/scalar_requires_satisfied_by_literal`, fail twin `scalar_requires_unproven_literal`),
- exit `ensures` for machines WITH bodies, via domain-fact flow (`domains/exit_ensures_unproven` and friends),
- bounded-type constraints (`omega-proof/src/checker.rs` interval checks on assignments, initializers, call arguments).

So the prover plumbing (facts, contexts, interval evaluator) exists; what is
missing is anchoring and discharging contract entailment for proof-artifact
machines.

## What Landed With This Roadmap

A contract refutation pass in
`compiler/semantics/omega-validation/src/contract_refutation.rs`. For
empty-body machines it rejects an `ensures` fact when:

- constant arithmetic disproves it (both sides fold to constants that compare
  false), or
- it states the strict-order reverse of a `requires` fact (asymmetry of `<`
  and `>`, with both spellings normalized).

Both checks gate on vacuity: if the `requires` set is itself visibly
contradictory (a constant-false fact, or a strict-order pair in both
directions), every `ensures` is vacuously true and nothing is rejected. This
is refutation only -- it never claims to PROVE an ensures, so unproven-but-true
theorems still pass unchecked. Active canaries:
`fail/proofs/constant_equation_refuted`, `fail/proofs/order_asymmetry_refuted`.

Known soundness limit of the refutation pass: requires-set consistency is only
checked syntactically (constant-false facts and direct strict-order pairs). A
requires set that is unsatisfiable in a form the pass cannot see would make a
refuted ensures vacuously true; tighten the vacuity gate as the engine's
satisfiability reasoning grows.

## The Ladder

True theorems live in `canaries/pass/proofs/` (all compile today). False twins
live in `canaries/pending/proofs/` registered as `CurrentlyAccepts` in the
canary suite -- they are the engine's acceptance tests. When an increment
lands and a twin starts rejecting, the suite panics with a promotion message;
move the directory to `canaries/fail/proofs/` (each pending twin already
carries an `expected.txt` placeholder fragment, `cannot prove ensures
contract`, to align with the existing exit-ensures prover wording -- update it
to the engine's real diagnostic when promoting).

| Rung | True theorem (pass/proofs/) | False twin | Engine increment required |
| --- | --- | --- | --- |
| L0 | `proof_constant_arithmetic_identity` (3*3 + 4*4 == 5*5 over Nat) | `fail/proofs/constant_equation_refuted` (ACTIVE) | constant folding + reflexivity; the refutation half is DONE, the proving half needs ensures anchoring for empty bodies |
| L1 | `proof_order_transitivity` (a<b, b<c -> a<c) | `fail/proofs/order_asymmetry_refuted` (ACTIVE, direct asymmetry); `pending/proofs/order_transitivity_false_twin` (3-variable cycle) | stored-fact matching + transitive closure of the order relation |
| L2 | `proof_linear_range_sum` (a,b in 1..=10 -> a+b in 2..=20) | `pending/proofs/linear_range_sum_false_twin` (upper corner 19) | interval arithmetic through `+` over requires-derived ranges (the interval evaluator in `omega-proof/src/checker.rs` already does this shape for bounded types; it needs to read contract ranges) |
| L3 | `proof_congruence_add_constant` (a==b -> a+1==b+1) | `pending/proofs/congruence_false_twin` (a+1==b+2) | rewriting under stored equations + constant normalization |
| L4 | `proof_addition_commutativity` (a+b == b+a) | `pending/proofs/addition_commutativity_false_twin` (b+a+1) | term normalization: canonical sum-of-monomials form |
| L5 | `proof_nonlinear_square_range` (a in 0..=10 -> a*a in 0..=100) | `pending/proofs/nonlinear_square_range_false_twin` (0..=99) | interval products (first nonlinear step; needs care with correlated operands -- a*a, not independent a*b) |
| L6 | `proof_order_antisymmetry` (a<=b, b<=a -> a==b) | `pending/proofs/order_antisymmetry_false_twin` (ensures a!=b) | combining non-strict order facts into equations |
| L6 | `proof_multiplication_distributivity` ((a+b)*c == a*c+b*c) | (covered by L4 normalization twin) | polynomial normalization (ring form) |
| L6 | `proof_remainder_range` (a%2 in 0..=1) | `pending/proofs/remainder_range_false_twin` (0..=0) | built-in euclidean lemmas (`Nat.mod_lt` analog) |
| L6 | `proof_bag_view_reflexivity` (Bag(items)==Bag(before) carried) | `pending/proofs/bag_view_false_twin` (ensures !=) | proof-view semantics: today `Bag(...)` is accepted surface with no lowering; even hypothesis-to-conclusion transport of an identical fact is unimplemented |
| L7 | `proof_inductive_gauss_sum` (2*gauss_sum(n, acc) == 2*acc + n*(n+1), tail recursion) | `fail/proofs/inductive_gauss_sum_false_twin` (ACTIVE, base arm disproved); `fail/proofs/inductive_gauss_sum_step_false_twin` (ACTIVE, recursive arm unprovable) | DONE 2026-06-12: per-arm obligations over transition bodies, `result` binding, induction hypothesis gated on a per-call-site strict-decrease discharge, hypothesis-bound subsumption in `prove_at_least` |
| L8+ | (none yet) | (none yet) | quantified facts; non-tail value recursion (blocked on frontend: compound arm expressions); statement-position recursion (blocked on termination graph coverage) |

Suggested landing order: anchor ensures obligations for empty-body machines
first (a `ContractExitFact` at statement_index 0 when the body is empty), with
the prover discharging L0 by constant folding and L1 by fact matching +
transitivity -- but DO NOT flip anchoring on before the prover can discharge
every pass/proofs rung, or the whole pass ladder (and
`constraints/proof_machine_order_fact`, `constraints/nat_proof_literal_suffix`)
turns red. Until then, extend the refutation pass instead; it is accept-by-
default and cannot strand true theorems.

## Frontend Acceptance Frontier (probed 2026-06-10)

Accepted in contracts today:

- `nat` literal suffixes (`3nat`), all comparison/arithmetic operators,
  parenthesized terms, `%`,
- range membership facts `x in lo..=hi` (including over compound terms:
  `a + b in 2..=20`),
- multi-fact contracts separated by newlines,
- `Bag(expr)` / `Seq(expr)` view calls over slice params (typecheck as facts),
- unknown function shapes like `min(a, b)` parse and validate as boolean facts
  (call expressions are assumed boolean-shaped) -- accepted but meaningless,
  a latent surface to tighten.

Rejected today:

- `Nat` as a parameter/data type ("references unknown data type `Nat`") --
  Nat exists only as a literal-suffix world, so theorems are stated over
  `usize` (machine ints, no overflow facts emitted yet),
- `Seq(items) in Sorted` ("references unknown domain `Sorted`") -- domains are
  declared over data types (`domain Type::Name`), there is no domain-over-view
  surface,
- quantifiers: `forall i in 0..items.len { ... }` is a parse error ("expected
  `;`, `,`, or end of proof facts") -- no quantified facts in contracts.

## Known Ceilings (unchanged by this roadmap)

- No quantifiers in contracts; sorting-style global facts are unstatable.
- Proof views (`Seq`/`Bag`/`Range`) are documented and parse, but have no
  lowering or semantics; `Sorted` and friends do not exist.
- `Real` approximation semantics are an open design question; float contract
  facts only have the bounded-type interval checks.
- Machine-int wrapping: contract arithmetic over `usize` does not yet emit
  overflow obligations, and the constant folder in the refutation pass uses
  checked i64 (it stands down on overflow rather than wrapping).
