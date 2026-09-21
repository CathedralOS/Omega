# DERIVATION-RECHECK-CACHE — scope verification (2026-09-20, `739e4e81e9`)

No board row under this literal name — the recheck-cache wiring slice was
assigned to and resolved under PROOF-SEARCH-DERIVATION-CACHE
(`TASKS.md` resolved row, landed `28a3cc7fea`).

## Landed surface (verified at HEAD)

- `proof/src/checker/derivation_cache.rs`: `ProofDerivationCache`
  consulted by each bounded certificate route inside
  `check_proof_plan_with_derivation_cache`; retained candidates under the
  obligation's canonical `ProofObligationKey` are re-decided through the
  admission kernel — accepted discharges the leg, rejected is tallied and
  passed over; kernel-accepted certificates are retained for later
  rechecks; `DerivationStoreFull` is explicit, never silent eviction.
- `DerivationCacheReport` tallies
  consultations/reused/rejected/retained/refused.
- Substrate: `derivation_store.rs` (PROOF-DERIVATION-STORE-INDEX, landed
  `68ce33d9de`); measurement via `OMEGA_PROOF_MEASUREMENTS`; residual
  dependency-invalidation leg owned by PROOF-CACHE-DEPENDENCY-INVALIDATION.

## Fresh witness attempt at `739e4e81e9` — blocked by sibling drift

`cargo test --no-run -p proof --lib` fails: `checker/measurement.rs`'s
test module lost `ProofNode`/`ProofRule` when `1bdaa86a8f7d` replaced its
glob self-imports with named lists (8× E0422/E0433, test-only). The file
is under RC-REPOSITORY-BASELINE-GREEN/glob-leg-2's live claim — attributed
in-flight repair surface, not repaired here. Prior witness on the landed
work: 81/81 `cargo nextest run -p proof` green on linux x86-64 including
four consultation tests in `checker/certificate/tests.rs`.

## Outcome

No independent slice exists — the item is discharged by
PROOF-SEARCH-DERIVATION-CACHE. Record only; coordinator may fold the
alias.
