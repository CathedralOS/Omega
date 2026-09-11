# Reusing proof search

Status: exploratory draft, not an accepted design or implementation task.
No measured proof-search bottleneck, persistent cache format, key, storage
location, or granularity is selected. These notes explore retaining derivations
to avoid repeating search. Replace them with a concrete proposal when a measured
workload justifies one, or delete them when no longer useful.

## Motivation and candidate approach

An unchanged obligation may repeat expensive proof search. Retain a successful
derivation and its complete statement/dependency context, then independently
check it against the receiving obligation before reuse. A cached success bit,
producer assertion, or successful test run is not proof evidence. Cache failure
is a miss or explicit resource refusal, never acceptance of an unchecked claim.

Keys should use normalized semantic identity, including exact selected laws,
assumptions, substitutions, and relevant schema/checker compatibility. A rename
should not invalidate a semantically identical obligation, but normalization
must not erase meaningful distinctions. Dependency changes invalidate affected
entries. A key computed from the proof term can identify a stored proof, but
does not alone provide the lookup key for finding it from an obligation.

Published [proof evidence](../spec/proofs/contracts.md) is a distinct reuse path:
its producer publishes a checked contract rather than substituting for search
still owned by the current consumer. Evaluator-result caches retain logical
usage under the [evaluation contract](../spec/language/evaluation.md).

## Alternatives and unresolved choices

- Recheck a published certificate without a search cache: fewer persistent
  mechanisms, potentially repeated search for local obligations.
- Store untrusted derivations in the existing content-addressed store with
  a local semantic lookup index; choose per-obligation or coarser granularity
  only after measuring realistic edit/recheck costs.
- A signed server verdict or cryptographic argument is a different admitted
  trust basis, not a faster implementation of unconditional local checking.

Large certificates can remain sound while exceeding a build's transfer,
storage, or checking budget. Size does not make them attestations, and a finite
search does not by itself supply a compact proof. Prefer compact checked
derivations where available without imposing an unproved universal size bound.
Measure hit rate, invalidation, total checking cost, and storage before selecting
a persistence scheme. No compiler-authored source rewriting or new lockfile is
required by this approach.
