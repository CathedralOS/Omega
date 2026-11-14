# Stable index identity and proved compatibility

Ratified 2026-11-06. Implementation remains open under
**OPERATOR-MACHINE-SUPPLY** in [TASKS.md](../../../TASKS.md).

Adding a valid algebraic law could previously stop two symbolic type arguments
from sharing a canonical identity. The selected law set and 64-term expansion
cutoff determined signatures, conformance matching and specialization keys.
The revised design keeps a small fixed identity profile and uses checked
algebraic equality to admit values across differently written indexed types.

## Decisions

- [Canonical identity](../../spec/proofs/contracts.md#stable-canonical-index-identity):
  transparent aliases, closed evaluation, and fixed per-core-operation
  associative/commutative/neutral-element rules. Preserve exact carrier,
  arithmetic policy and selected operation. No blanket integer algebra.
- User laws, including associativity, commutativity, literal connections and
  distributivity, prove compatibility without changing keys. A checked
  proof may be discovered automatically or supplied through ordinary citation.
- No expansion in identity and no mutual-distributivity exception. The
  canonical profile/schema remains published and versioned; search policy is
  separate provenance, not another identity dialect.
- The root Build setting `builder.proof.index_expansion_terms` is a positive
  `u64`, default `64`. It bounds summands per attempted proof expansion.
  [Policy and usage](../../spec/build/configuration.md#proof-work-policy) are
  recorded under sponsor authority. Other resource ceilings remain independent.
- Preserve successful smaller-budget discoveries when this allowance increases,
  with the other inputs and sufficient resource grants unchanged. Published
  proofs and generic derivations travel without re-search. Checking evidence
  still owes its own trust and resource admission.
- Canonical matching remains proof-search-free. Proof compatibility cannot select
  a conformance, infer an ambiguous index, merge declarations or rename a
  specialization. Shared expressions/aliases support APIs needing exact identity.
  Distinct symbolic keys may emit distinct specializations; fully closed equal
  values still share keys. Backend deduplication remains optional.
- No new generic argument, cast requirement, language keyword, implicit algebra,
  runtime proof operation, or polynomial source language.

This is a semantic migration, not a claim of a current wrong-code bug.
The existing spec already allowed compatibility evidence beyond the canonical
cutoff. The change removes an unstable source of declaration identity; it does
not invent transport or promise that every algebraic equality is decidable.

## Implementation route

1. Inventory exact core operation/profile rows, carriers and policy qualifiers.
   Implement fixed canonical simplification separately from selected user-law
   discovery. Preserve the source operation and its independent formation proof.
   Refuse unsupported input rather than making keys depend on a partial search.
2. Remove law-sensitive expansion and user-law simplification from
   `open_index_identity.rs` and all identity producers/consumers. Migrate the
   current `OpenIndexOperationSelection` evidence into compatibility proving;
   do not discard selected-operation or conformance-application authority.
   Keep typed canonical ordering, schema provenance, and closed evaluation
   consistent through inference, specialization and publication.
3. Use the ordinary prover at value admission. Check every rewrite's exact
   operation, law instance, premises and formed operands. Literal connections
   and ordinary theorem guarantees remain available. Carry derivations through
   substitution and Checked/Terminal evidence interfaces without re-search.
4. Add the Build field, normalized proof policy, sponsor accounting and recorded
   search limits. Resolve policy for staged product discovery without allowing
   an unevaluated Build setting to authorize its own admission. Monotone search
   allowance must be an implemented scheduling property, not an assertion about
   an arbitrary budget-dependent heuristic.
5. Publish and verify compatibility evidence separately from canonical keys and
   discovery receipts. Update source/artifact identity versions and codecs
   together. Reject or explicitly migrate legacy artifacts; do not reinterpret
   old law-expanded keys under the new profile. No wire-tag numbers are reserved
   by this plan.

## Acceptance

Reuse the existing `open_index_distributivity` sample and its nine transport
places and the `literal_linked_index_identity` fixtures. Keep original value,
custody and execution oracles while migrating proof ownership. The sample's
[historical results](../../../samples/cli/facts/open_index_distributivity/README.md)
are not new validation of this design.

- **Stable keys:** equivalent core AC/unit forms share keys; distributively
  related symbolic forms and user-law-only equalities retain separate keys.
  Vary the proof budget and add a valid reverse law without changing the
  canonical expression normalization. Preserve ordinary changed-contract
  dependencies and exact selected operation identity.
- **Policy correctness:** signed saturating reassociation must not be inferred
  from a primitive carrier; Wrapping preserves its modulus; Exact retains every
  authored intermediate overflow obligation. No key simplification or new proof
  expression bypasses formation. Keep foreign-algebra, false-law, missing-law,
  and undischarged-premise controls.
- **Compatibility:** automatic transport through local, field, parameter,
  call-result, join, const-name, nested-machine, other-machine and target-body
  placements uses retained evidence. Include noncommutative associative chains
  and computed index substitutions; no source-shape recognizer or special
  caller guard may replace the general mechanism.
- **Mutual laws:** a valid forward proof remains usable after adding the reverse
  law. Audit the old `mutual_distributivity` negative; rejection solely due to
  the retired canonical exception is no longer desired behavior. Record any
  reclassification only with actual source and execution evidence. Conditional
  laws remain usable only when their premises are proved.
- **Budget boundaries:** exercise 64 and 65 resulting terms, explicit larger
  allowance, theorem-assisted equality beyond the allowance, and a large
  well-formed type that needs no equality proof. Failure reports unresolved
  equality/search exhaustion, not inequality. Increasing the allowance preserves
  previous successes and never changes keys or selected declarations.
- **Exact matching:** automatic value compatibility does not make differently
  spelled symbolic conformance/signature applications identical. Shared aliases
  and generic common indices work; otherwise ambiguous inference still rejects.
  Closed constant evaluation and optional code deduplication stay separate.
- **Separate consumption:** produce a library proof with a larger discovery
  allowance and consume it with a smaller one, checking retained evidence once
  at the trust boundary. Exercise altered subjects, operations, premises,
  substitutions and malformed certificates as negatives. Keep verification
  exhaustion separate from discovery exhaustion and production receipts separate
  from consumer verification charges.
- **Carry-through:** validate the same admitted programs through the lowered-form
  checker, serialized/reloaded Terminal Psi, interpreter and native execution
  on hosted targets.

The old `bounded-canonical-index-normalization` and
`literal-linked-index-identities` decisions are superseded/narrowed in
[proof contracts](../../spec/proofs/contracts.md#licensed-normalization).
The operator-introducer retirement and qualified operator-shape lowering in the
same board item remain independent unfinished obligations.
