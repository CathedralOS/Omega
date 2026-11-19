# Required automatic discharge

Ratified 2026-11-10. In Omega an unproved obligation rejects, so what a
checker proves without help decides which programs it accepts. The spec fixed
what counts as a proof (kernel-checked certificates) and how search is
budgeted, but not which proofs a checker must find. Two checkers could
therefore accept different programs with neither being wrong. The spec now
names a floor every checker discharges unaided:

```omega
machine get(items: &[u8], i: u64, n: u64) -> u8
requires i < n
requires n <= items.len
{
    items[i]             // order reasoning: i < n <= items.len
}

machine area(count: u32, size: u32) -> u32
requires count <= 1000
requires size <= 1000
{
    count * size         // range propagation: at most 1,000,000
}

machine halves(i: u64, j: u64, n: u64) -> bool
requires 2 * i <= n
requires n <= 2 * j
{
    i <= j               // true, but outside the floor: cite a lemma
}
```

The normative owner is
[required automatic discharge](../../spec/proofs/contracts.md#required-automatic-discharge).
Track the work under `REQUIRED-AUTOMATIC-DISCHARGE` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. The floor is defined by what is derivable from the premises in scope, not
   by a search algorithm or a named theory.
2. Order reasoning is in the floor and is complete: comparisons between
   subjects, literals and subject-plus-literal terms, through any chain of
   transitivity, equality substitution and the step between `<` and `<=`.
3. Range propagation is in the floor: bounds flow once, in definition order,
   through add, subtract, multiply, divide, remainder, shift and cast, and
   discharge representability and nonzero-divisor goals.
4. Case splits on relevant disjunctive premises are in the floor up to 64 arm
   combinations per obligation.
5. The floor has no search allowance and is outside the proof-work policy.
   Both parts are polynomial in the size of the machine body.
6. A floor discharge produces the existing kernel certificates. No
   certificate rule is added by this decision.
7. Three outcomes are reported: accepted; "does not follow from the premises
   in scope" with a counterexample assignment, for order goals; and not
   established, for everything else.
8. A checker may prove more than the floor. `omega --check --floor-only`
   restricts it to the floor and lists what needed more.

## Why this fragment

The existing certificate rules already are order reasoning plus range
propagation: `IntegerStrictOrderTransitivity`, `IntegerOrderSubstitution`,
`IntegerMathOrderTranslation`, `IntegerOrderDiscreteness`,
`IntegerCarrierBound` and their neighbours for the first, and
`IntegerAffineBound` for the second. Defining the floor as their closure needs
no kernel change and matches what source relies on today.

The self-hosted checker is built the same way. `source/psi/checking/facts.omg`
records an integer comparison between linear terms as a difference fact and
otherwise bounds a term by a value's interval, and `arithmetic.omg` propagates
intervals through operations. Both checkers already exist and already use
these two mechanisms, each with its own reach, which is the divergence this
decision closes.

Full linear integer arithmetic was the other candidate. It would leave out
`count * size`, which is not linear, and it would require combination and
rounding steps (`2 * i <= n` and `n <= 2 * j` give `i <= j`) that no rule
expresses.

## Work

1. Audit that every floor derivation has a certificate under the existing
   rules, including atoms with a literal offset and comparisons across
   carriers. A gap is closed by a rule, not by shrinking the floor.
2. Make the Rust checker complete for order reasoning and emit the
   counterexample assignment on a definite rejection.
3. Bring the self-hosted checker in `source/psi/checking` to the same floor.
   It reasons with difference facts and intervals today and does not name the
   integer certificate rules, so its coverage is measured against the corpus
   below.
4. Add `--floor-only`.
5. Add the shared corpus below.

## Acceptance

- A must-accept corpus passes on both checkers under `--both` with no cited
  lemma: transitivity chains of length 2, 8 and 64; equality substitution
  through a projection; `a < b` used as `a + 1 <= b`; mixed signed and
  unsigned subjects; add, subtract, multiply, shift and cast representability
  from operand ranges; a nonzero divisor from a range; a case split over a
  two-arm and a three-arm premise.
- A must-reject corpus is rejected by both with "does not follow" and a
  counterexample: a missing middle link; a strict bound used where a
  non-strict one was proved; an index bounded by the wrong length.
- A beyond-the-floor corpus, including `halves` above, is reported as not
  established under `--floor-only` on both.
- `source/library/core`, `source/library/std`, `source/psi` and
  `source/omega` pass `--check --floor-only`, or each reported obligation
  cites a lemma.
- A floor obligation is accepted with `builder.proof.index_expansion_terms`
  set to 1.

## Left out

General linear combinations and window instantiation. Either joins the floor
when the obligation corpus shows it is common and its certificates exist.
