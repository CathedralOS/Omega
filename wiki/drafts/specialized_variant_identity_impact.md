# Specialized-variant identity impact

Purpose: record how future specialized variants would interact with code
identity, deduplication, and component replacement — one of the open research
questions in [learned optimization](learned_optimization_policy.md). This is an
analysis of existing identity machinery, not an authorization for a
variant-emitting pass or a new identity kind. Delete these notes when a
producer that emits variants lands or the referenced machinery changes.

Affected subjects: [optimization selection and validation](../spec/build/optimizations.md)
and [artifact verification](../spec/terminal-psi/verification.md).

## What exists today

- An optimization unit's identity is a canonical digest over its complete
  content (`optimization-unit/src/optimization_unit/identity/unit_encoding.rs`):
  vocabulary marker, program fingerprint, fuel schedule, entry, structural
  types and domains, services, boundary machines, provider candidates,
  obligation facts, proof questions, ownership frontier, pruned machines, and
  functions. Any specialization changes canonical bytes, so a specialized unit
  carries a distinct `OptimizationUnitIdentity` automatically, while identical
  replays produce identical identities — deduplication of equivalent outputs
  is free.
- The specialization chain binds those identities honestly. The independent
  validator re-realizes the plan and recomputes the output identity and the
  candidate identity from it
  (`abstract-operations-to-abstract-operations/src/state_specialization/validate.rs`);
  application rejects stale candidates (`StaleCandidateRevision`) and output
  mismatches (`OutputIdentityMismatch`), and the transformation ledger records
  rule, candidate, validator, input and output (`apply.rs`). Replay cannot
  adopt a forged variant identity, and the candidate identity itself binds the
  machine, dispatch, and specialized-edge roster.
- `MachineFunctionIdentity` (`function-identity`) has three kinds — `Source`,
  `ProgramStorageEntryWrapper`, and `CallbackThunk` — each retaining the source
  `StateKey` continuation through `associated_source_continuation`. No kind
  exists for a specialized variant.
- Emission identity digests the complete fragment plan
  (`machine-code/src/machine_code/fragments/identity.rs`), so differently
  specialized code yields different fragment emission identities; component
  descriptions carry `realization_identity` and `component_description_identity`
  (`component-description`).

## How specialized variants would interact

### Code identity

A specialized variant's unit identity is already distinct and deterministic.
What does not exist is a per-function identity for a variant: a pass emitting
two functions from one source machine could not reuse `Source` without
colliding on the same `StateKey`, and a fabricated label would violate the
rule that generated functions name closed compiler-owned roles. Such a kind
must bind the producing specialization's coordinates — the rule and candidate
identity or an explicit specialization index — beside the continuation, so two
variants of one source differ, identical specialization replays converge, and
`associated_source_continuation` keeps source lineage.

### Deduplication

Identical canonical content already dedups at the unit level by construction.
The object plan's `function_symbols` arena is keyed by handle, not identity,
and `object_function_symbol` (`object-file/src/names/mod.rs`) resolves an
identity to one symbol and fails closed on duplicates — so a variant sharing
`Source` identity with its origin would not silently conflate, it would
reject both lookups. The roster dedup in `component-description` operates on
identity lists where a variant must appear as a distinct entry, again
requiring the distinct kind above.

### Component replacement

Replacement operates on component descriptions, obligations, and realization
identities rather than per-function names, so a specialized component keeps a
distinct `realization_identity` already. Replacement compatibility needs a
written answer for whether a specialized realization may replace an
unspecialized one sharing its interface — an admissibility question for the
component contract, not for the identity machinery, which only has to expose
the distinction honestly.

## Remaining questions

- Which specialization coordinates a variant kind must bind so distinct
  variants never collide and replay stays deterministic.
- Whether private symbol naming (`object-file/src/names`) extends to variant
  identities or variants use pinned names like callback thunks.
- Whether replacement compatibility keys on source lineage
  (`associated_source_continuation`) plus realization identity, or needs a
  variant-aware contract.
