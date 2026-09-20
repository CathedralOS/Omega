# PROOF-DERIVATION-STORE — scope verification (2026-09-20, `c93cceb9ca`)

Board row `TASKS.md:9450` already reads "Resolved — covered by owned
sibling rows. No independent slice remains under this name." Verified
live: the stub re-mines `wiki/drafts/proof_search_cache.md`'s
derivation-store leg, which is already delivered and decomposed.

## Decomposition map (all verified at HEAD)

| Leg | Surface | Status |
| --- | --- | --- |
| Store substrate | `proof/src/derivation_store.rs` — content arena of
  untrusted derivation payloads indexed by canonical
  `ProofObligationKey`, generational `DerivationId`, explicit
  `DerivationStoreFull` capacity refusal, key-granularity `invalidate` |
  landed `68ce33d9de` (PROOF-DERIVATION-STORE-INDEX) |
| Candidate-only consult | `check_proof_plan_with_derivation_cache` in
  `checker/derivation_cache.rs` — re-decides retained candidates through
  the admission kernel, `DerivationCacheReport` tallies | resolved
  `28a3cc7fea` (DERIVATION-RECHECK-CACHE) |
| Measurement | `OMEGA_PROOF_MEASUREMENTS` — one `key=value` line per
  `omega --check` run | resolved (PROOF-SEARCH-MEASUREMENT) |
| Open residual | dependency invalidation | owned by
  PROOF-CACHE-DEPENDENCY-INVALIDATION |

## Fresh witness

`cargo nextest run -p proof --lib derivation_store` — 6/6 PASS at
`c93cceb9ca` (linux x86-64): semantic-identity lookup, insertion-order
candidates, invalidation frees key row + stales ids, stale ids from
another store do not resolve, explicit capacity refusal,
clear-drops-everything.

## Outcome

No code change — record only. Coordinator may drop the stub.
