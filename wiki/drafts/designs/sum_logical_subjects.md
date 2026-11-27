# Sum inputs in logical contracts

The owners ruled on 2026-11-27 that a sum input to a logical contract is one
mathematical value of the sum's checked inductive declaration. It is not a
flattened tuple of case Booleans and payload scalars. A contract may test the
active case, read a payload field under that case's knowledge, compare whole
sums with `==` and `|proof|::Identity`, and pass the sum to mathematical
definitions. The same rule covers non-recursive and recursive sums.
[Ruling](../../spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)

This note plans the implementation. It is a draft, not specification.

## What exists

- **Theorem contracts are scalar.** `terminal_psi::TheoremContract` has one
  `ValueDeclaration` per subject. A record input is flattened into one subject
  per scalar leaf by `terminal_semantics::theorems::data_subjects`. That
  flattening stays as it is.
- **Denotation is scalar.**
  `proof_admission::denote_bounded_implication_type` binds each subject as a
  scalar carrier and denotes `Proposition`s over `ScalarTerm`s.
- **Proofs are scalar.** The bounded prover builds `ProofNode` certificates
  for scalar and propositional logic. No rule eliminates an inductive value.
- **Mathematical data already elaborates.** Stage 05's mathematical signature
  (`read/signature.rs`) turns a source sum into the kernel's `PlainData`. That
  gives its canonical trees, one definition per case and an induction
  principle. It also reads laws over such data, and stage 06 proves them from
  their citations. The kernel validates the block (`inductive.rs`).

## Plan

Each step is one PR. Each positive test has a negative twin, and every
unsupported shape is refused by name.

1. **Contract shape.** `TheoremContract` gains data subjects. Each has a place
   and a sum type, and clauses name it the way record formals are named:
   `StructuralCaseMembership` for a case test, a field term under a
   `Case(c), Field(f)` path for a payload read, and an identity clause for a
   whole-sum comparison. The wire codec and its format marker change
   together. Validation refuses a data subject by name until step 2 can
   denote it.
2. **Denotation.** For each data subject, elaborate the sum's declaration to
   `PlainData` declarations in the contract's signature, then bind the subject
   as a value of that type. A case test and a guarded payload read denote
   through the recursor. `==` denotes as the kernel's identity. The carrier
   key covers the declaration, so two contracts over different sums never
   share a proof type.
3. **Stage 05.** Read a sum formal on a theorem alias and on a cited machine
   into a data subject. Record inputs stay flattened.
4. **Citation.** A cited machine with a sum formal supplies its contract over
   the same data subject. Where the implication needs a split on the case,
   the prover concludes `c in S::A || c in S::B || ...`, naming every case
   once, by the rule `StructuralCaseExhaustion`. The kernel derives it by
   induction over the sum: the motive is that disjunction of case tests,
   and at each case its own test computes to `0 = 0`. The rule is checked
   only over a theorem's sum input, where the declaration is bound. The
   cited machine must still return without running anything.
5. **Application.** Where the law is applied, the checker builds the actual
   value as the constructor of the case known at the edge, applied to the
   payload values reaching it. The premise then reduces to scalar
   propositions about those values. An application whose case is unknown at
   the edge is refused by name. A recursive sum has no runtime layout, so
   only mathematical bodies apply a law to one.

## Risks

- The case-analysis rule is new proof vocabulary (proof-system marker 10).
  The kernel derives each use through the recursor; it is not an axiom.
- Step 1 changes the wire format. Committed `.psi` fixtures that carry the old
  format marker need regenerating.
- Whole-sum identity is only a proposition the contract states. It does not
  give a structural comparison substitution for a sum with an IEEE float
  payload ([contracts](../../spec/proofs/contracts.md)).
