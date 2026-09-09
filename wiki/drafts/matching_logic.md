# Exploring matching-logic proof interchange

Purpose: retain research background and possible comparison methods for matching
logic. This is not a concrete integration proposal or authorization for a new
calculus, kernel replacement, or implementation experiment. Delete these notes
when superseded by a concrete design or no longer useful. Related subjects:
[proof contracts](../spec/proofs/contracts.md) and
[artifact verification](../spec/terminal-psi/verification.md).

## Motivation

An independent proof/semantics route could cross-check Omega obligations or
translate external proofs. Matching logic is a candidate, not a promise of a
smaller trusted base. Compare the complete current route with the candidate's
translation, theory, checker, and soundness bridge; checker size alone is not
the cost or trust argument.

Chen and Rosu's [2026 paper](https://arxiv.org/abs/2608.13306v1) proves global
completeness for one-sorted finitary basic matching logic without fixpoints.
Adding least fixpoints makes validity non-recursively-enumerable, excluding
a sound effectively enumerable calculus complete for all such validities.
This does not prevent sound checking of finite certificates in an intentionally
incomplete fragment. Its many-sorted counterexample concerns the studied
calculus; it does not establish non-axiomatizability of every fixpoint-free
many-sorted calculus. Omega's nominal declaration identities are not the
hybrid-model nominals discussed by the paper.

## Possible bounded comparison

An informative comparison would use one real vertical slice with scalar
propositions, equality, quantification, a Terminal state transition, and a
reconstructed refinement obligation. The
artifact verifier derives the theory and goal from canonical subjects; a proof
producer cannot supply its own weaker question.

Measure checker, translation, and theory size; certificate size and checking
time; and every imported rule, assumption, or trusted bridge. Compare identical
pinned positive and negative cases. Preserve the distinction between all-model
consequence, the intended mathematical model, and operational refinement.

Then examine:

- Classical versus constructive reasoning. Do not silently import classical
  rules into a different accepted foundation or disguise them as translation.
- One inductive carrier's finite base/step/decrease certificate and its intended
  model. Unrestricted least-fixpoint proof search is not a prerequisite.
- Typed-to-one-sorted encoding of `Nat`, `Int`, `addr`, slices, and a user sum:
  sort membership, definedness, junk models, revisions, borrows, and multiplicity.
- One small external arithmetic proof with its exact source axiom closure and
  checked proof-object translation. Semantic expressibility alone is insufficient.

All evidence retains logical fragment, rule/semantics versions, exact subject,
target capsule, observation profile, bridge graph, and admissions. The paper's
completeness theorem supplies no Omega authority merely by being cited.

## Possible outcomes and alternatives

A future comparison might motivate an untrusted proof producer, an independent
semantic cross-check, or a proof-import route. A checked source proof with a trusted
translation retains a translation admission; an imported statement alone is a
foreign-theorem admission. Neither is independently checked translation.

Keeping the existing route is valid if the alternative does not improve total
audit cost or capability. Replacing the kernel needs a separate proposal and an
end-to-end proved bridge, not just a smaller pattern checker. Encoding semantics
as axioms relocates trust rather than discharging it. Failed search never proves
falsity, and no completeness promise for every valid program is implied.
