# MATH-PROOFS-DECLARATION-SELECTION

Re-verified at `90df29812c00` (linux x86-64). Mined stub: eighth re-mine of
the adjudicated TRAIT-MATHEMATICAL-PREDICATE-CONTRACTS surface — chapter 14's
recorded gap that a trait requirement expressing an arbitrary nondecidable
validity condition routes through a mathematical predicate parameter
([mathematical_bindings](../spec/proofs/mathematical_bindings.md)), with
"checking that general route remains implementation work" and ownership by
PROOF-CONTRACT-MIGRATION.

"Declaration selection" names the leg where a requirement's predicate
parameter (`P: A -> core::Strict<v>`) binds a selected mathematical
declaration at a satisfying conformance or call.

## State at this tip

- The declaration substrate is landed and green:
  `symbol-resolved-trees-to-typed-trees` lowers authored `let`/`boundary let`
  mathematical declarations into the typed mirror
  (`typed_trees::mathematical::{MathematicalDefinition,MathematicalParameter,MathematicalType}`);
  `typed-trees-to-checked-trees/src/proof/mathematical_declarations.rs`
  records checked declarations, and `proof/mathematical_signature.rs`
  elaborates each to a `mathematical_core::signature::Declaration` and runs
  kernel `check_signature` — including explicit/generic applications,
  generalized level inference, machine-call denotations, and premise-carrying
  mathematical calls. Lane: `cargo nextest run -p typed-trees-to-checked-trees
  -E 'test(~mathematical)'` → 68/68 PASS.
- The selection leg does not exist in the checked representation:
  `typed_trees::declarations::trait_definition::TraitRequirement` carries only
  `lifetime_arguments` plus `arguments: HandleSpan<TypeReferenceHandle>` —
  there is no mathematical-parameter slot on a requirement, so no
  conformance/call site can yet select a declaration into a predicate
  parameter. That is the connected implementation PROOF-CONTRACT-MIGRATION
  owns, not a bounded fix on this stub.

## Fences

All fences the prior verifications named have drained: MATH-FOUNDATION-BINDINGS
(mathematical_signature/declarations), PROOF-CONTRACT-MIGRATION
(proof_output_calls + pass/proofs), and PROOF-KERNEL-CORE (proof-admission
wholesale) all expired on the live claim map. Residual fences nearby:
PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION holds validation's
contract_entailment/specification_calls.rs + refuted_requires.rs +
call_requirements.rs, and NEW-QTL-INVALID-LAW-FAIL-TWINS holds two
fail/proofs fixtures. None covers the requirement-selection surface — but no
independent slice exists regardless: adding a mathematical argument slot to
`TraitRequirement` is the canonical item's design surface.

Sibling re-mines verified the same way: MATHEMATICAL-PREDICATE-PARAMETERS,
MATHEMATICAL-FOUNDATIONS-REAL, MATH-PROOFS-CALL-SELECTION-OCCURRENCE.
