# Classicality audit of the proof rules

This document is the audit of every proof rule an accepted certificate can
cite against the classical/constructive boundary that the
[matching-logic exploration](../../drafts/reference/matching_logic.md) requires:
classical reasoning must not be silently imported into the accepted
foundation or disguised as translation.

The classification is executable, not only prose:
`AcceptedProofRule::foundation`
(`omega-rust/psi/compilation/proof/admission/src/classicality.rs`) assigns each
rule a `ProofRuleFoundation` by exhaustive match, so a new certificate rule
does not compile until it is classified. The module's tests pin the classification:
no rule may map to `Classical`, and only `SemanticAxiom` is a `TrustedAdmission`.
This is a review inventory, not a proof that the implementations satisfy their
assigned logical properties. Rule implementations, conversion, premise handling
and trusted admissions require their own soundness justification.

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
  relation. No admitted rule maps here.

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
| `IntegerMathOrderTranslation` | Constructive | `a <= b` implies `a + t <= b + t` over mathematical integers, derived from the existing addition monotonicity law. It introduces neither a new assumption nor a finite operation. |
| `IntegerOrderAntisymmetry` | Constructive | Two opposite nonstrict integer bounds establish equality by the existing order antisymmetry law. No excluded middle or new assumption is introduced. |
| `IntegerLessOrEqualTransitivity`, `IntegerStrictOrderTransitivity` | Constructive | Order composition with an exact shared middle; strict chains require at least one strict edge. |
| `IntegerOrderSubstitution` | Constructive | Replaces exactly one endpoint through a proved equality; never changes strictness. |
| `IntegerAffineBound`, `IntegerExactAddDefinitionBound`, `IntegerExactSubtractDefinitionBound`, `IntegerCastBound`, `IntegerCorrelatedForbiddenRoots` | Constructive-decidable | Checked witness conversion over the decidable fixed-integer fragment; each accepted use re-records the witness's cited semantic-axiom roster — they carry trusted admissions as premises, never as silent steps (`cites_semantic_axioms`). |
| `IntegerRemainderWithinDividend` | Constructive-decidable, **provisional** | The `IntegerAffineBound` form for an exact remainder by a divisor that is not a literal: `min(a, 0) ≤ a % d ≤ max(a, 0)` once `0 < d` or `d < −1` is proved. The statement is trusted, not derived by the mathematical core; it was admitted under the `kernel-rule-admission` decision in the [foundation](foundation.md), is checked against computed arithmetic on every pair of `i8` and `u8` operands, and cites its result equation like the other bound rules. `AcceptedProofRule::is_provisional` marks it. |
| `IntegerQuotientAtMostDividend` | Constructive-decidable, **provisional** | The `IntegerAffineBound` form for an exact quotient by a divisor that is a value: `a / d ≤ a` once `L ≤ a` for a literal `L ≥ 0` and `0 < d` are proved. Trusted, not derived by the mathematical core; admitted under the same `kernel-rule-admission` decision, checked against computed arithmetic on every pair of `i8` and `u8` operands, and citing its result equation. `AcceptedProofRule::is_provisional` marks it. |

## Structural boundary

- Negation is definable without a primitive connective: `not P` means
  `P -> Falsehood`. The proposition grammar can therefore express excluded
  middle, `P OR (P -> Falsehood)`, and double-negation elimination,
  `((P -> Falsehood) -> Falsehood) -> P`. Expressibility does not supply a
  derivation. These principles are not licensed for arbitrary propositions
  without supporting premises; decidable instances can have constructive
  proofs, and a supplied classical premise remains an explicit dependency.
- Contradiction elimination (`ex falso`) is constructive: a checked proof of
  `Falsehood` can establish any well-formed goal under those same premises.
  Predicate-denotation conversion and conjunction elimination can express
  this derivation. See [contradiction elimination](#contradiction-elimination).
- Recursion admission (`admission/recursion.rs`) is well-founded
  induction: a named ranking relation, a proved well-foundedness
  obligation, and a per-edge decrease certificate. This is the
  base/step/decrease certificate the matching-logic lane calls out, and it
  is constructive.
- The mathematical core is a predicative Π/Σ calculus with stratified
  relevant/strict universes — no cumulativity, no self-typing universe,
  predicative formation only. Its checked theorems (e.g. identity
  transport through `J`) are ordinary re-decided definitions, not axioms.

## Contradiction elimination

Named decision: `scoped-contradiction-elimination`, settled 2026-11-03.

A proved contradiction establishes that no execution satisfies the premises of
that path. For example, an entry requirement `count >= 10` and a branch condition
`count < 5` cannot both hold for the same value. Obligations on that arrival may
therefore close by contradiction elimination. This is ordinary constructive
reasoning, not classical proof by contradiction or an unchecked escape.

The contradiction must be independently checked from valid premises about the
exact values and storage versions in scope. Failed proof search does not prove
falsehood. Local branch assumptions and their conclusions cannot escape their
scope or justify another reachable arrival; joins retain the ordinary
all-predecessor obligations. Existing well-formedness, operand formation,
ownership and authority boundaries are not waived.

The language permits contradiction elimination without prescribing its encoding.
One derivation converts a checked `Falsehood` to `Falsehood AND goal`
through their equal bounded predicate denotations, then eliminates the
conjunction. Both the contradiction and all its premise dependencies remain
checkable. An explicit elimination rule would require the ordinary rule audit,
not a change from constructive to classical logic.

Proving an arrival impossible, removing its emitted instructions and warning
about unreachable source are separate decisions. Neither code pruning nor a
warning is required to make the proof valid. Diagnostic policy is specified
under [unreachable branches](../language/state_contracts.md#unreachable-branches).

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

Acceptance requires a checked derivation under the exact recorded premises.
A classical formula may appear as a goal or supplied premise without changing
the grammar. Citing such a premise does not turn its conclusion into an
unconditional theorem or make the assumption constructive.

The classification requires each inference rule to have a constructive
justification, including any decidability assumptions. Exhaustive classification
makes new rule variants visible to review; it cannot detect a defect in an
existing rule, conversion algorithm or trusted admission. The combined calculus
and its interpretation require the justification specified by the
[foundation](foundation.md) and [kernel metatheory](kernel_metatheory.md).

## Obligation formation and proof search

Collecting a contract site or constructing an obligation identifies what must be
proved; neither supplies a derivation. Proof-search helpers produce candidate
evidence for the classified rules above. A reported site, helper label or failed
search must not be treated as an accepted fact. Boundary premises still require
the explicit admissions described above.

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

Certificate-gated rows require checked derivations using the classified rules
and retain their premise dependencies. Trusted rows record their admissions.
The ledger exposes those routes for review; its exhaustiveness does not prove
that reconstruction or rule implementations preserve the claimed boundary.
