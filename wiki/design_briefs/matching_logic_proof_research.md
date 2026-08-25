# Design Brief: Matching Logic as a Proof and Semantics Research Lane

Scouted 2026-08-24. Status: THEORETICAL -- open research, no mechanism or
kernel migration chosen.

This note records what the 2026 completeness results for basic matching logic
do and do not imply for Omega. It is deliberately separate from the settled
[proof-kernel architecture](../architecture/bootstrap_lattice/proof_kernel.md).
The current kernel, canonical semantic ledger, and artifact-verification route
remain authoritative while this lane is investigated.

Primary reference: Xiaohong Chen and Grigore Rosu,
[Completeness and Incompleteness of Basic Matching Logic](https://arxiv.org/abs/2608.13306),
arXiv:2608.13306v1 (2026).

## Result relevant to Omega

For one-sorted, finitary, fixpoint-free basic matching logic, the paper gives a
globally complete calculus: for arbitrary theories `Gamma`, semantic global
consequence and derivability coincide.

The sharp negative result is different. After adding least fixpoints (`mu`),
validity is not recursively enumerable, already over a small signature and the
empty theory. Therefore no sound calculus with an effectively enumerable proof
relation can prove every valid matching-`mu` formula.

This limits completeness, not sound certificate checking. A finite certificate
in a sound, intentionally incomplete calculus can still be checked. Failure to
find or check a proof remains rejection or unknown; it never establishes that
the proposition is false.

Two qualifications prevent over-reading the paper:

- Its many-sorted counterexample defeats the studied localization-based
  calculus. It is not a second non-axiomatizability result for every effective
  many-sorted calculus; fixpoint-free many-sorted forms retain first-order
  routes.
- Structural induction may be interpreted using least-fixed-point semantics,
  but Omega can continue checking finite base/step/decrease certificates without
  exposing unrestricted object-level `mu` or searching all matching-`mu`
  validities.

The hybrid-logic nominal result is not a result about Omega's nominal
declaration identity. It matters only if an eventual encoding represents an
Omega identity as a hybrid model-fixed world name.

## Why this is interesting for Omega

Matching logic can describe values, configurations, binders, operational rules,
and other logics as theories over one small pattern calculus. That makes it a
plausible common certificate or semantic-interchange language across proof
producers. A matching-logic checker could also provide an implementation path
independent of Omega's current generic kernel.

That promise is not a proof that the total verification base becomes smaller.
A fair comparison includes all of:

```text
current route
    canonical Omega semantics
    + obligation reconstruction
    + current proof calculus and checker
    + soundness bridge

matching-logic route
    canonical Omega semantics
    + obligation reconstruction
    + Omega-to-matching-logic translation
    + matching-logic theory
    + matching-logic checker
    + theory/translation soundness bridge
```

Encoding semantics as axioms relocates trust unless the artifact verifier
reconstructs the encoding and the correspondence to pinned Omega meaning is
proved. Checker source size alone is therefore not an acceptance metric.

The paper's displayed calculus is classical. Omega's bootstrap kernel is
constructive by default and treats excluded middle as an explicit admitted
boundary claim. Importing a classical matching-logic calculus silently would be
a semantic-policy change, not a checker refactor.

## The owner question that precedes the experiment

Omega must first say what kind of semantic consequence artifact verification
needs:

```text
all-model consequence
    every model satisfying an Omega theory satisfies the obligation

initial/intended-model consequence
    the obligation holds in the initial or otherwise intended Omega model

canonical operational judgment
    the reconstructed artifact judgment holds in one pinned transition system
```

These are not interchangeable. The paper proves global completeness for the
first shape in its restricted fragment. Characterizing an intended initial
model may require no-junk or least-fixed-point machinery. Directly constructing
one canonical operational model may make global completeness irrelevant while
leaving matching logic useful as a notation or secondary checker.

This decision is tracked as Q2 in
[`OWNER_QUESTIONS.md`](../../OWNER_QUESTIONS.md).

## Bounded investigation

If Q2 leaves a genuine matching-logic customer, investigate in this order.

### 1. Encode one fixpoint-free vertical slice

Translate a small real slice containing scalar propositions, equality,
quantification, one Terminal Psi state transition, and one reconstructed
artifact-refinement obligation.

The artifact verifier, not the proof producer, must derive the matching-logic
theory and goal from canonical subjects. Measure checker size, translation and
theory size, certificate size, checking time, and every imported logical axiom.

Acceptance: the current route and the experimental route check the same pinned
positive and negative corpus, and the experiment states the exact trusted
bridge rather than reporting checker line count alone.

### 2. Resolve classical versus constructive reasoning

Determine whether the useful fragment has an adequate constructive calculus.
If it does not, classical reasoning remains an explicit scoped admission or the
matching-logic route remains a non-authoritative cross-check.

Acceptance: no classical tautology enters an authoritative Omega derivation
without an identity-bearing rule or admission visible to artifact review.

### 3. Prove one induction correspondence

For one Omega inductive carrier, connect an accepted structural-induction
certificate -- base case, induction step, exact recursive edge, and strict
decrease -- to the carrier's intended least-fixed-point meaning.

Acceptance: the bridge needs no unrestricted matching-`mu` proof search and
makes no completeness claim. Any later `mu`-bearing certificate identifies its
fragment explicitly. The aconjunctive/reachability fragment is a legitimate
separate research candidate because the paper's negative construction does not
settle it.

### 4. Test the typed-to-one-sorted cost

Encode a small typed set such as `Nat`, `Int`, `addr`, a byte slice, and one user
sum. Record how sort membership, partiality/definedness, no-junk constraints,
borrows, revisions, and multiplicity are represented.

Acceptance: the experiment identifies junk or ill-sorted models, every axiom
used to exclude them, and whether those axioms cross into least-fixed-point or
initial-model reasoning.

### 5. Test one external proof import

Translate one small Lean or Rocq arithmetic theorem with its exact source axiom
closure. Mere semantic definability of a type theory in matching logic is not
enough; the customer is a checkable proof-object translation with measured
certificate cost.

Classify the result honestly:

- independently reconstructed and checked translation: discharged obligation;
- checked source proof but trusted translation: scoped translation admission;
- imported theorem statement only: ordinary foreign-theorem admission.

## Potential landing shapes

The low-risk candidates are:

1. **Untrusted proof producer.** A matching-logic prover searches and emits a
   certificate consumed by an authoritative Omega checker.
2. **Independent semantic diamond.** The current and matching-logic routes
   independently reconstruct and check the same Terminal Psi obligation.
3. **Proof interchange/import lane.** External proof systems translate selected
   theorems into a disclosed, checkable intermediate form.

Replacing the bootstrap proof kernel is not an initial experiment. It becomes a
candidate only if an end-to-end comparison shows a smaller total trusted base,
acceptable certificates, and a proved bridge to Omega's chosen semantic
subject.

## Standing constraints

- Omega promises sound checking for supported certificate rules, never
  completeness for every semantically valid program or theorem.
- `mu`, induction, recursion, and reachability are not one undifferentiated
  feature. Explicit well-founded certificates remain the default.
- Proof search, translation optimization, and certificate generation remain
  untrusted.
- Certificate identity records the logical fragment and rule-semantics version.
- A proof bundle never chooses its own artifact obligation or semantic theory.
- Inability to prove is rejection or unknown, never falsity.
- The paper's completeness theorem is not part of Omega's trusted argument
  unless it is mechanized and actually used; soundness of accepted derivations
  remains the first requirement.
