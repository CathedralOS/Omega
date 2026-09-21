# TRAIT-MATHEMATICAL-PREDICATE-CONTRACTS — scope verification (2026-09-21, `75650d2e94`)

Re-verification of the adjudicated mined stub at `75650d2e94` on linux
x86-64. The 2026-09-20 verdict stands: no independent slice exists under
this name — the surface is PROOF-CONTRACT-MIGRATION's connected
implementation.

Board hygiene note for the coordinator, not a board edit: the row appears
three times in `TASKS.md`; two copies carry stamps that belong to other
items (a CTTL-FAILURE-ATTRIBUTION / KNOWN-BASELINE-FAILURES-REFRESH
sentence and a TERMINATION-RANK-RANGE-FIELDS `field_endpoint` re-witness).
The canonical adjudication is the copy that cites chapter 14.

## The recorded gap

`wiki/language_guide/chapter_14_traits.md` still carries both sentences:

- "General predicate parameters, logical binders, and noncomputable
  mathematical values remain to be specified."
- "An arbitrary nondecidable validity condition uses a
  [mathematical predicate parameter](../spec/proofs/mathematical_bindings.md),
  not an implicitly substituted Boolean decider. Checking that general
  route remains implementation work."

## Coverage map at `75650d2e94`

- **Requirement slot absent by construction.**
  `omega-rust/psi/representations/typed-trees/src/typed_trees/declarations/
  trait_definition.rs::TraitRequirement` carries `symbol`, `name`,
  `lifetime_arguments`, `arguments: HandleSpan<TypeReferenceHandle>` and
  `source_span` — no mathematical parameter slot. The s2t lowerer
  (`symbol-resolved-trees-to-typed-trees/src/lowering/trait_definition.rs`)
  fills exactly those fields; neither syntax trees nor resolution have a
  predicate-parameter surface to select.
- **Declaration substrate landed, as a different node kind.**
  `typed_trees::evidence::mathematical::MathematicalDefinition` carries the
  top-level `let`/`boundary let` telescope into typing;
  `typed-trees-to-checked-trees/src/proof/mathematical_declarations.rs`
  elaborates to `CheckedMathematicalDeclaration` and
  `proof/mathematical_signature.rs` produces a kernel-checked signature.
  `checked-trees-to-lowered-psi` still refuses them with the named
  PROOF-CONTRACT-MIGRATION diagnostic (fail canary
  `proofs/mathematical_declaration_lowering_rejected`) pending Terminal
  evidence encoding. None of this is reachable from a `requires` clause.
- **Owning item.** PROOF-CONTRACT-MIGRATION owns "contract proof
  semantics, Terminal evidence/codec/replay, and core mathematical
  traits"; its acceptance case 2 is a higher-order theorem over arbitrary
  mathematical predicates. Declaring a `P: A -> core::Strict<v>`
  requirement on a trait needs that declaration, signature and evidence
  machinery — it is the canonical item's design surface, not a bounded
  slice.
- **Fences at verification time.** The claims prior stamps named —
  PROOF-CONTRACT-MIGRATION, PROOF-KERNEL-CORE, MATH-FOUNDATION-BINDINGS —
  have all drained; no live claim covers `typed-trees`, the `t2c/src/proof`
  surface, `proof-admission`, or `source/library/core`. The only adjacent
  live claim is PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION on
  `validation/src/proof_contracts/contract_entailment` plus
  `tests/omega/{pass,fail}/proofs` pins (exp 16:19Z), a different surface.

## Sibling re-mines carrying the same adjudication

MATHEMATICAL-PREDICATE-PARAMETERS, MATHEMATICAL-FOUNDATIONS-REAL,
MATH-PROOFS-CALL-SELECTION-OCCURRENCE, MATH-PROOFS-DECLARATION-SELECTION
(record: `wiki/drafts/math_proofs_declaration_selection.md`).

## Outcome

No code change — record only. The coordinator may fold the stub into the
PROOF-CONTRACT-MIGRATION cluster; the next acceptance for this surface is
its migration-example case 2.
