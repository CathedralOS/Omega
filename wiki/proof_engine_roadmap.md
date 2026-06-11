# Proof Engine Roadmap

Status doc for the requires->ensures entailment engine. This maps the theorem
ladder in `canaries/pass/proofs/` and `samples/math_proofs/` to the engine
increments each rung needs, and records what the checker actually discharges
today (2026-06).

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
| L7+ | (none yet) | (none yet) | induction via recursive contracts + `decreases`; quantified facts |

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
