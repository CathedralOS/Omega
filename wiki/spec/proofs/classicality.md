# Classicality audit of the proof rules

This document is the audit of every proof rule an accepted certificate can
cite against the classical/constructive boundary that the
[matching-logic exploration](../../drafts/reference/matching_logic.md) requires:
classical reasoning must not be silently imported into the accepted
foundation or disguised as translation.

The classification is executable, not only prose:
`AcceptedProofRule::foundation`
(`omega-rust/psi/semantics/proof-admission/src/classicality.rs`) assigns each
rule a `ProofRuleFoundation` by exhaustive match, so a new certificate rule
does not compile until it is classified. `ProofLemma::foundation` and
`ForAllInRangeFact::ELIMINATION_FOUNDATION`
(`omega-rust/psi/semantics/proof/src/lemmas.rs`) hold the obligation-side
lemma vocabulary to the same rule. The module's tests keep the
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
| `IntegerAddOrder` | Constructive-decidable | `r = a + b ∧ 0 < b ⊢ a < r` over exact fixed integers. |
| `IntegerSubtractAntitone` | Constructive-decidable | `p = m − x ∧ q = m − y ∧ y < x ⊢ p < q` over exact fixed integers with one minuend. |
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

## Obligation-side lemma library and deciders (`psi/semantics/proof`)

These are obligation checkers and premise planners, not certificate rules:
a checker decides an obligation against declared facts, and whatever it
cannot discharge becomes a diagnostic rather than an accepted admission.

| Surface | Foundation | Note |
| ------- | ---------- | ---- |
| `ProofLemma::{IndexInBounds, NonEmptyHasFirst, WindowLength, WindowSubrange, TailLengthDecreases}` | Constructive-decidable | Closed length/bounds/window relations over `usize`-shaped carriers; each lemma fires only when every premise in `premises()` is already established (`lemmas.rs`). `ProofLemma::foundation` enforces the row by exhaustive match — a new lemma does not compile until it is classified. |
| `ForAllInRangeFact` / `QuantifiedBound` / `ElementIndex` | Constructive-decidable | Element discharge (`proves_element`, `contains_index`, `is_vacuous`) compares literal bounds only; symbolic bounds answer conservatively — never a decision over an undecidable relation. `ForAllInRangeFact::ELIMINATION_FOUNDATION` pins the elimination step to this row. |
| `checker/{arrival_stability, assignment_stability, bounded_checks, dependent_bounds, float_ranges, guards, integer_ranges, named_constraints, return_arrival}` | Constructive-decidable | Total deciders over the checked finite shapes: range arithmetic, arrival joins, guard narrowing and named-constraint lookup are each decidable relations over declared data. |
| `checker/certificate/` | Constructive-decidable over an untrusted input | Bounded-integer legs arrive as untrusted certificates that the proof-admission kernel re-decides; the *decision* is decidable, and the cited rule rows keep their own classification from the table above. |
| `checker/measurement.rs` | N/A (instrumentation) | Proof-search cost accounting; discharges nothing. |
| `obligations/{collection, constraints, identity, plan, program_queries, range_tests, ranges}` | N/A (plan formation) | Walks the typed program into obligations and derives/readbacks constraints; produces what must be proved, not inference. |
| `boundary.rs` | Trusted admission (premise declaration) | Models what a host/provider primitive must establish and preserve — the obligation side of `EvidenceRoute::Admitted`; the obligations themselves are declarations, never proved. |
| `proof_surface.rs` | N/A (report) | Collects declared proposition/domain/contract sites for reporting. |

No lemma or decider performs case analysis over an undecidable relation,
introduces a negated premise, or assumes a fact the roster does not carry.

## Verifier semantic-axiom reconstruction inventory

Which reconstructed facts may be cited through `ProofRule::SemanticAxiom`
is the trusted-list question; the inventory is the trusted-surface ledger
(`terminal-verifier/src/trusted_surface/reconstruction.rs` + `checker.rs`),
already exhaustive per `ReconstructedTerminalObligationOwner`,
`Terminator` and `ReconstructedFactKind` variant. Each row's `soundness`
records `ExplicitlyTrusted` — this audit classifies *why* each is
trusted, and which rows are not trusted at all.

| Inventory member(s) | Foundation | Note |
| ------------------- | ---------- | ---- |
| `owner:{scalar-block-invariant, operation, call-requires, nominal-cleanup-requires, contract-ensures}` | Trusted admission | Reconstruction authority over *which* obligation exists; each produces a derivable obligation proposition to discharge, not an inference. |
| `terminator:{jump, conditional, structural-case, return, return-unit, return-unit-partial-affine, return-unit-nominal-affine, return-structural, crash}` | Trusted admission | Edge/exit axiom transport and per-clause obligation reconstruction; the exit sets carry incoming axioms exactly. |
| `fact:semantic-axiom-roster` | Trusted admission | The ordered deduplicated proposition set `SemanticAxiom` indices cite; dominance-order traversal decides membership, not equality. |
| `fact:goal-free-scalar-result`, `fact:scalar-carrier-bounds`, `fact:integer-structural-field-read-range`, `fact:field-store-leaf-equation`, `fact:structural-case-arm` | Trusted admission | Emit the result-equality, carrier-bound and establishment propositions that join the roster — the axiom rows a certificate cites. |
| `fact:call-{parameter-instantiation, requires-instantiation, ensures-import}`, `fact:content-partition-composition`, `fact:successor-parameter-binding` | Trusted admission | Call-frame premise transport: parameter-binding equations and rewritten ensures/requires join the axiom set by reconstruction authority. |
| `fact:boolean-polarity-implications`, `fact:successor-path-transport`, `fact:branch-condition-transport`, `fact:header-invariant-members` | Constructive (certificate-gated) | Each emitted fact stands on its own canonical certificate accepted by the proof checker before it may join a premise roster — a re-decided derivation, not a trusted premise. |
| Licensed premise introductions inside `successor-path-transport` and `branch-condition` | Trusted admission | The explicitly enumerated residues — the unrejected fixed-shape restatement, literal-adjacency disequality strengthening, equal-terms unsatisfiable arm falsehood, and the backward-only boundary truth — are admitted premises, listed on the row. |
| `fact:borrowed-storage-restoration-debt`, `fact:crash-site-retention`, `fact:proof-bearing-scalar-goal`, `fact:record-establishment`, `fact:scalar-case-establishment`, `fact:byte-extent-length`, `fact:structural-effect-observation`, `fact:return-result-binding` | Trusted admission | Reconstructed debt/retention/goal propositions and establishment facts; the canonical-goal row fixes which proposition a producer may choose. |

The boundary holds on this surface too: the certificate-gated rows are
the only non-trusted emissions, and they rest on the already-classified
certificate rules (`semantic-axiom`, `assumption`, `predicate-denotation`,
`equality-transitivity`, `implication-introduction`) — classical content
could only enter through a new trusted row, which the exhaustive ledger
bindings force into review.
