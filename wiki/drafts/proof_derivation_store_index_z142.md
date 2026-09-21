# PROOF-DERIVATION-STORE-INDEX — verification record (2026-09-20, `8e870505f7`)

Marked row at `TASKS.md:6312` already **Resolved — landed at
`68ce33d9de`**. Re-verified at
`8e870505f7d644fa407c1e5ad24eaa13b0b1ded4` (linux x86-64, cargo):

- `git merge-base --is-ancestor 68ce33d9de origin/main` — landed.
- `proof/src/derivation_store.rs` is the lookup substrate
  `wiki/drafts/proof_search_cache.md` requires: content arena of untrusted
  derivation payloads indexed by canonical `ProofObligationKey`
  (BTreeMap, deterministic iteration), generational `DerivationId`
  handles, explicit `DerivationStoreFull` capacity refusal,
  key-granularity `invalidate`, candidate-only lookups re-decided by the
  admission kernel.
- Fresh witness: `cargo nextest run -p proof --lib derivation_store`
  **6/6 PASS** (insertion order, invalidation frees key row + stales ids,
  semantic-identity lookup, explicit capacity refusal, cross-store stale
  ids, clear).

## Fences / residual

- Item held by sibling `Devin / z175-proof-derivation-store-index`
  (exp 23:34Z, Resolved-annotation leg) — this leg claimed overlap for
  verify+record only.
- Consultation inside `check_proof_plan` belongs to DERIVATION-RECHECK-CACHE
  (resolved, `28a3cc7fea`); open residual = dependency invalidation under
  PROOF-CACHE-DEPENDENCY-INVALIDATION.

Verdict: board row already resolved and landed; no code change.
Coordinator can drop the stub.
