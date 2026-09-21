# Matching-logic vertical slice

Board items: `MATCHING-LOGIC-VERTICAL-SLICE` (candidate checker leg of
`MATCHING-LOGIC-BOUNDED-SLICE`). Implements the bounded vertical slice
drafted in
[wiki/drafts/matching_logic.md](../../wiki/drafts/matching_logic.md)
("Possible bounded comparison"): one slice with scalar propositions,
equality, quantification, a Terminal state transition, and a refinement
obligation the checker reconstructs from the canonical subject.

## What it is

A certificate checker for the one-sorted finitary basic fragment (no
fixpoint symbols — the fragment the cited completeness result covers).
It is deliberately a checker, not a prover: a producer supplies a finite
derivation tree; this tool re-derives every node against the declared
theory and compares the conclusion to the goal it reconstructed itself.
A producer cannot answer a weaker question, and every theory clause it
consumed is reported as an axiom admission — nothing here is admitted
authority over Omega proofs.

## Surfaces

- `slice_checker.py check <case.json>` — verify one case; exit nonzero
  when a positive case rejects or a negative case accepts.
- `slice_checker.py record` — run every pinned case and write
  `record.json` (`omega-matching-logic-slice-record/1`): checker size,
  rule inventory, per-case verdict/certificate bytes/check ms — the
  candidate-side columns the comparison record consumes.
- `cases/` — `reference.json` (positive: `exists s'. step(s0,s') /\`
  `in(counter(s'),Nat)` derived from transition + membership axioms) plus
  negatives rejecting a weaker goal, an undeclared axiom, an undefined
  witness, an escaping eigenvariable, a wrong-side equality substitution,
  a half-proven membership body, and a reversed transition.
- `tools/tests/test_matching_logic_slice.py` pins the suite locally.

## Slice content vs the doc's checklist

Scalar propositions (`pred`), equality (`equals` + subst rules),
quantification (`exists`/`forall` with definedness and eigenvariable
discipline), a Terminal state transition (`step` axiomatized per case),
and the reconstructed refinement obligation (`build_goal`). Imported
rule/assumption inventory: `admissions` in every verdict, all of it
axiom admissions — there is no trusted bridge to measure.
