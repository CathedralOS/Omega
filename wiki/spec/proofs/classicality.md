# Classicality audit of the proof rules

This document is the audit of every proof rule an accepted certificate can
cite against the classical/constructive boundary that the
[matching-logic exploration](../../drafts/matching_logic.md) requires:
classical reasoning must not be silently imported into the accepted
foundation or disguised as translation.

The classification is executable, not only prose:
`AcceptedProofRule::foundation`
(`omega-rust/psi/semantics/proof-admission/src/classicality.rs`) assigns each
rule a `ProofRuleFoundation` by exhaustive match, so a new certificate rule
does not compile until it is classified. The module's tests keep the
boundary: no rule may map to `Classical`, and only `SemanticAxiom` is a
`TrustedAdmission`.

## Foundations

- **Constructive** — intuitionistically valid inference; the conclusion is
  witnessed from checked premises.
- **Constructive-decidable** — constructive only because the checked
  relation is decidable over its carrier (closed integer relations, carrier
  bounds, discreteness of fixed integers, bounded denotation, checked
  witness conversion). Deciding a decidable relation is not excluded
  middle, but generalizing such a rule to an undecidable domain would be a
  disguised classical step and requires reclassification.
- **Trusted admission** — not an inference rule; cites a
  verifier-reconstructed premise that is itself trusted input.
- **Classical** — reserved: excluded middle, double-negation elimination,
  proof by contradiction, choice, or a decision over an undecidable
  relation. Nothing maps here today.

## Certificate rules (`terminal_psi::ProofRule` → `AcceptedProofRule`)

| Rule | Foundation | Note |
| ---- | ---------- | ---- |
| `Primitive` | Constructive-decidable | `PrimitiveJudgment` re-decides Truth, reflexive equality, closed integer relations and carrier bounds over closed data (`kernel.rs`). |
| `SemanticAxiom` | Trusted admission | Cites one verifier-reconstructed axiom proposition; recorded on the acceptance (`propositional_rules.rs`). |
| `Assumption` | Constructive | Cites a scoped premise; ambient and discharged-local rosters are tracked by `traversal.rs`. |
| `ConjunctionIntroduction`, `ConjunctionElimination` | Constructive | Standard ∧-I/∧-E over the checked conjunct children. |
| `DisjunctionIntroduction`, `DisjunctionElimination` | Constructive | ∨-I injects a proved disjunct; ∨-E is case analysis with each branch checked under its locally pushed disjunct assumption. |
| `ImplicationIntroduction`, `ImplicationElimination` | Constructive | →-I discharges exactly the pushed premise on exit; →-E is modus ponens against a checked implication. |
| `EqualitySymmetry`, `EqualityTransitivity` | Constructive | Symmetry and exact-shared-middle composition of proved equalities. |
| `PredicateDenotation` | Constructive-decidable | Definitional conversion through the bounded predicate denotation; both sides are re-denoted and must land on one goal. |
| `ValueEqualityTransport` | Constructive | Substitution along independently proved value equations — the J-rule shape; ambient premises unchanged. |
| `IntegerOrderWeakening` | Constructive | `a = b` or `a < b` entails `a ≤ b` — definitional weakening. |
| `IntegerOrderDiscreteness` | Constructive-decidable | `x ≤ c` to `x < c + 1` by literal adjacency; sound because the fixed-integer order is discrete and decidable. |
| `IntegerSubtractOrder` | Constructive-decidable | `r = a − b ∧ 0 < b ⊢ r < a` over exact fixed integers. |
| `IntegerLessOrEqualTransitivity`, `IntegerStrictOrderTransitivity` | Constructive | Order composition with an exact shared middle; strict chains require at least one strict edge. |
| `IntegerOrderSubstitution` | Constructive | Replaces exactly one endpoint through a proved equality; never changes strictness. |
| `IntegerAffineBound`, `IntegerExactAddDefinitionBound`, `IntegerCastBound`, `IntegerCorrelatedForbiddenRoots` | Constructive-decidable | Checked witness conversion over the decidable fixed-integer fragment; each accepted use re-records the witness's cited semantic-axiom roster — they carry trusted admissions as premises, never as silent steps (`cites_semantic_axioms`). |

## Structural boundary

- The proposition language has **no negation connective** and no
  quantifiers: `Proposition` (`semantic-vocabulary`) carries Truth,
  Falsehood, atoms, equality/order atoms, domain atoms, conjunction,
  disjunction and implication only. Excluded middle and double-negation
  elimination are not merely absent from the rule set — no certificate can
  state them.
- `Falsehood` has **no elimination rule**: there is no ex falso in the
  certificate fragment, so the calculus is stricter than intuitionistic on
  that point.
- Recursion admission (`admission/recursion.rs`) is well-founded
  induction: a named ranking relation, a proved well-foundedness
  obligation, and a per-edge decrease certificate. This is the
  base/step/decrease certificate the matching-logic lane calls out, and it
  is constructive.
- The mathematical core is a predicative Π/Σ calculus with stratified
  relevant/strict universes — no cumulativity, no self-typing universe,
  predicative formation only. Its checked theorems (e.g. identity
  transport through `J`) are ordinary re-decided definitions, not axioms.

## Trusted admissions outside the rule set

These are trusted premises or extensions, not inference rules; each is
recorded on its acceptance for audit:

- `SemanticAxiom` citations (above) — verifier-reconstructed propositions.
- `EvidenceRoute::Admitted` — `ForeignBoundaryGuarantee`, `ProviderFact`
  and `CheckedAssemblyClaim` (`terminal-psi` `proof_bundle/admission.rs`):
  host/provider claims admitted under an installation-profile decision.
- The set-quotient interface (`mathematical_core/quotient.rs`): seven
  explicitly admitted named mathematical assumptions (`Q`, `project`,
  `setQ`, `sound`, `effective`, `elim`, `beta`) per the
  [quotient specification](quotients.md), with the remaining interface
  derived by checked terms. Set-quotients are compatible with a
  constructive foundation, but they enter as named admissions and stay
  visible as such.

## Boundary statement

Every certificate rule currently admitted is constructive outright or
constructive by domain decidability; the only trusted content enters
through recorded admissions, and the proposition grammar cannot express a
classical principle. A classical rule therefore cannot arrive silently:
it would need a new `Proposition` connective and a new `ProofRule`
variant, which the exhaustive `foundation` classification forces into the
audit before it compiles.

Remaining surface this audit does not yet classify: the obligation-side
lemma library and deciders in `psi/semantics/proof` (obligation checkers,
not certificate rules), and the verifier's semantic-axiom reconstruction
inventory (which axioms may be reconstructed is a separate trusted-list
question).
