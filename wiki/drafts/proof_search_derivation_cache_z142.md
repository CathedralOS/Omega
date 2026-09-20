# PROOF-SEARCH-DERIVATION-CACHE — verification record (2026-09-20, `d74f2145b9`)

Marked row at `TASKS.md:9490` already reads **Resolved**. Verified at
`d74f2145b96f2e2bdbeffbf8679837da11b65d66` (linux x86-64, cargo):

- `checker/derivation_cache.rs` carries `ProofDerivationCache` +
  `DerivationCacheReport` — caller-owned, entered via
  `check_proof_plan_with_derivation_cache` (`checker.rs:66`), re-exported at
  `checker.rs:36`.
- Each bounded certificate route consults candidates retained under the
  obligation's canonical `ProofObligationKey`; candidates are re-decided
  through the admission kernel — accepted discharges the leg, rejected is
  tallied and passed over; kernel-accepted certificates are retained for
  later rechecks. Capacity refusal is explicit `DerivationStoreFull`.
- Landed as `7a417dbab3b9` ("proof: consult a derivation cache in
  check_proof_plan certificate routes"; earlier record cited `28a3cc7fea`).

Fresh witness: `cargo nextest run -p proof --lib -E 'test(/recheck|derivation|retained/)'`
— **8/8 PASS**: `retained_certificate_re_decides_for_a_semantically_identical_obligation`,
`recheck_misses_an_obligation_with_no_retained_candidates`, and the 6
`derivation_store` tests (capacity refusal, semantic-identity lookup,
key-granularity invalidation, cross-store staleness, clear, insertion order).

## Fences / residual

- Item-level claim held by sibling `Devin / dev-88738-proof-search-derivation-cache`
  (exp 00:16Z) on `derivation_store.rs`, `derivation_store/`, `checker.rs`,
  `wiki/drafts/proof_search_cache.md` — this leg claimed overlap for
  verify+record only.
- `PROOF-DERIVATION-STORE` (sibling resolved row) still held by Jarod
  (exp 02:01Z); dependency invalidation lives under
  PROOF-CACHE-DEPENDENCY-INVALIDATION.

Verdict: board row already resolved, behavior verified and re-witnessed
green; no code change. Coordinator can drop the stub.
