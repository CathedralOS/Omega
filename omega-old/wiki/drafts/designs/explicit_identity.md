# Explicit identity and consistent equality

Owner-ratified 2026-11-06: value `==` retains its selected executable comparison
in contracts; `core::Identity<T>(a, b)` explicitly exposes kernel identity.
Contracts then state the same comparison a body evaluates. Separate checked
laws justify moving between the relations without changing either's meaning.

Normative owners: [proof contracts](../../spec/proofs/contracts.md#data-identity-in-facts),
[core equality acquisition](../../spec/language/conformances.md#core-equality-acquisition),
[foundation](../../spec/proofs/foundation.md), and
[float relations](../../spec/terminal-psi/mathematical_values.md#float-equality-and-source-identity).
**PROOF-KERNEL-CORE** in [TASKS.md](../../../TASKS.md) owns implementation and
source migration. This is an accepted design, not an open proposal or a claim
that the new declaration and compiler support are already delivered.

## Implementation sequence

1. Expose a sealed core mathematical declaration for `Identity<T>(a, b)` with
   the existing proof-relevant `Id` formation/elimination, exact type/universe
   binders and source custody. Naming the relation does not prove it. Do not
   introduce a keyword, `===`, a runtime Boolean identity algorithm or arbitrary
   proposition-returning machines. Retain proof-only subjects without requiring
   executable equality or duplicating linear runtime resources.
2. Remove contextual identity elaboration from value `==`. Preserve selected
   operator/implementation identities and ordinary proof-term admission,
   including selected preconditions. Missing equality diagnoses the missing
   operation and suggests explicit identity when appropriate. Do not silently
   choose identity or synthesize an unavailable comparison for generic data.
3. Supply independently selectable evidence traits for comparison-implies-identity
   and identity-implies-comparison. Each has complete operation and theorem rows;
   a combined trait may require both. Consume laws only for the exact equality
   row selected at the use, with its type arguments, premises and assumption
   closure. Recognize checked law structure rather than its spelling. Explicit
   theorem citation remains an equivalent evidence route, not ambient discovery.
4. Extend authorized synthesis with checked fieldwise comparison transparency
   and each independently justified bridge. Preserve tag/case guards, field
   operation selections, field preconditions and totality obligations. Do not
   make user comparisons structural by assumption or grant an empty arbitrary
   conformance compiler privileges. IEEE float fields prevent unconditional
   bridge synthesis; record padding is not part of value identity.
5. Preserve congruence for repeated admitted pure comparisons on the same exact
   values. A precondition about a comparison can prove its unchanged result
   without identity evidence. Retain mutation versions and reject stale,
   observation-dependent or differently selected call substitutions.
6. Align raw IEEE carrier identity with exact same-format bits, including NaN
   encodings/payloads/signs and signed zero. Keep IEEE comparison and payload-erased
   `FloatMeaning` identity separate. Update proposition construction, kernel
   interpretation, lowering, receiving verification and any required wire schema
   together. An old opaque `StructuralIdentity` row is not evidence that this
   interpretation already exists, and meaning equality cannot recover erased bits.

## Migration and acceptance

Inventory every contract/proposition equality formerly elaborated as identity,
retaining source site, concrete or abstract type, selected new comparison and
available bridge evidence. Mark migration safe only when both directions of
equivalence and new invocation admission are established. A comparison-implies-
identity law alone does not show that an old identity premise still implies the
new Boolean premise. Abstract carriers require uniformly available evidence,
not facts about one instantiation. Review remaining sites explicitly.

Rewrite identity-bearing algebra laws, including `source/library/core/algebra.omg`
and its satisfiers, to explicit `core::Identity`. Include the `core/int.omg`
integer-pair relation and proof-meaning wrappers in the inventory. Do the same for literal-linked
zero/one laws and any generic normalization licenses. Law recognizers must
consume actual identity evidence, never treat arbitrary `equal(...) == true`
as a rewrite permission. Do not edit bundled source to unsupported syntax before
the producer/core route exists. Update guides and implementation notices at closure.

Reuse existing equality and proof fixtures; add only focused non-`facts/`
fixtures where required. Acceptance includes:

- A record precondition proves the same pure comparison returned by the body;
  mutation and changed-comparison controls reject reuse of the fact.
- Synthesized record equality yields field comparisons; sum equality yields
  only guarded payload consequences. User equality supplies no unproved fields.
- Integer records support both bridges; case-insensitive comparison can support
  reflection without identity implication; an always-true comparison cannot
  license substitution or generic algebra rewrites.
- Same-NaN reflexivity for identity, IEEE nonreflexivity, different NaN payloads,
  and signed zeros retain the three specified relations through scalar and
  aggregate paths. Projection equality cannot prove raw bit identity.
- Mathematical laws without executable equality use explicit identity; `==`
  without a selected comparison rejects. Ill-formed/forged core identity,
  mismatched comparison rows, missing laws and circular proofs reject.
- The semantic migration covers source, checked evidence, Terminal verification
  and matching-host native execution. Exercise existing nine-place witnesses
  with both engines and retain exact provenance across package boundaries.

Measure proof-checking and generated-code effects; this ratification claims
neither a mechanical-only migration nor a measured speedup.
