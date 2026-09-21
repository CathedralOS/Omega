# Producer/checker decision-sharing audit

Audit of every surface where a producer-written decision, annotation, or
recorded disposition reaches a checker or consumer that must not trust it.
Covers the mined `PRODUCER-CHECKER-*` cluster (BOUNDARY-AUDIT,
DECISION-SEPARATION, DECISION-SHARING-AUDIT, SHARING-AUDIT) — those items
name the same seam family and this ledger holds their union.

Verified at revision `12dea522b2` (2026-09-20, Linux x86_64) by reading the
cited code and its existing negative tests; no new test was needed because
every reachable seam already has a pinned rejection. Verdicts:

- **Re-derived** — the checker recomputes the claim from bytes/inputs and
  the producer row is advisory evidence only.
- **Bound** — the shared value is accepted only under an exact identity
  match the producer cannot widen (comparison digest, subject fingerprint,
  profile membership).
- **Informational** — recorded for humans/audit, no code path converts it
  into authority.

## Surfaces

| Producer writes | Consumer | Mechanism | Verdict |
|---|---|---|---|
| Lock `decisions` section (`omega-policy-decisions` v2 document) | Later lock readers / resume | `HistoricalPackagePolicyDecisions` keyed to `source_subject`; `PackageLockTarget::from_policies` rejects `DecisionSourceMismatch`; the only readers are budget accounting and `inspect_packages` reporting — no path converts history into authority | Informational + Bound |
| Review decision document (`decision ... pending` tokens) | `recover_package_policy_review` | Template is re-rendered from *fresh* `changes`; every non-`decision` line must match byte-for-byte (`ChangedFindings`), the `comparison` header must name the fresh fingerprint, and `resolve_package_policy_decisions` binds each choice to a required subject of that comparison (`WrongComparison`, `UnknownSubject`, `NonBlockingChange`, `DuplicateComparisonSubject`, `MissingDecision`, `TooManyDecisions`) | Bound |
| Retained policy rows beside lock decisions | `review/compare/locked_policy.rs` | Fresh compiler review set is compiled and compared against the retained target; `MissingReview`/`UnexpectedReview`/`ResolutionMismatch`/`PackageIdentityMismatch`/`TargetMismatch`/`PurposeMismatch`/`ExecutionProfileMismatch` reject — "neither role borrows the other role's acceptance" | Re-derived |
| PCC sidecar claim fields (product, artifact commitment, semantic/checker profiles, guarantees+premises, assumptions, dependencies) | `verify_pcc_claim_fields` | Artifact commitment recomputed over the real bytes; every claimed profile/guarantee/premise/assumption/dependency is checked against the *receiver's* `PccReceiverPolicy`; the producer cannot select a weaker question | Bound + Re-derived |
| Placed-image evidence (text/data/import extents, region inventories, import-thunk bindings, entry address) | `NativePlacedImageEvidence` checker legs | Extents sliced out of committed bytes, digests and both inventory seals recomputed, thunk opcodes re-derived against the closed form, loader-visible entry re-derived from container bytes | Re-derived |
| Component description (rosters, exports, realization rows, custody) | `verify_component` | "The description is trusted for nothing": re-decode, subject reconstructed rather than read, terminal-verifier runs under the *consumer-supplied* admission profile, every module-evident row re-derived | Re-derived |
| Admission evidence (site, evidence identity, profile decision id) | `verify_obligation` | Accepted only if the exact triple is present in the receiver's `AdmissionProfile` acceptances — profile decisions are receiver-side state, not producer claims | Bound |
| Retained-artifact replay parts (selected plans, digests, custody) | `RetainedNativeArtifact::from_replayed_parts` and sibling rejoin checks | Substituted or inconsistent producer identities fail closed ("native artifact nearest-FMA does not rejoin one exact selected provider plan"); every retained row rejoins exactly one authority row | Re-derived |
| Trusted-surface digest ledger (`terminal-verifier/src/trusted_surface/`) | Its own coverage test | Producer-side ledger pins trusted implementation digests; any edit without revalidation fails `recorded_digests_match_the_working_tree`. Self-audit machinery, not a producer→checker claim — included to delimit the boundary | Bound |

## Findings

1. **No surface converts a recorded decision into authority.** Lock-stored
   policy decisions are history: capture validates the exact
   subject/comparison, recovery validates framing and source binding, and
   the authority path always re-resolves against the *fresh* comparison
   fingerprint. A stale saved document cannot satisfy a changed comparison
   (pinned: `package_policy_changes/document.rs` wrong-comparison and
   `ChangedFindings` tables).
2. **A recorded `reject` can never publish.** `capture_policy` preserves
   rejections as history; "merely recording one cannot permit its
   publication" (`lock/decisions/model.rs`).
3. **Producer annotations exist only as claims to be replayed.** The PCC
   native evidence documents each producer-supplied locator as validated,
   not trusted; the checker recomputes every extent, digest, seal, thunk
   and entry address from committed bytes.
4. **The receiver selects the policy everywhere.** Package acceptance
   decisions, PCC `PccReceiverPolicy`, component-verification admission
   profile, and proof `AdmissionProfile` are all consumer-supplied; no
   producer annotation narrows what is checked.

## Residual notes (not defects)

- `HistoricalPackagePolicyDecisions.comparison` is informational history:
  recovery stores it without binding it to the sibling policy rows' provenance.
  Safe under today's read set (budget + inspection only); if a future consumer
  ever consults it for authority, it must be bound the same way the fresh
  `resolve` path binds decisions to `changes.fingerprint()`.
- The PCC native product still reports `Incomplete` for instruction-row
  semantics, edges, premise availability and lowering correspondence — an
  honest coverage boundary, not a decision-sharing hole.
- A future proof-derivation store (`proof_search_cache.md`) will introduce a
  new sharing surface; its own rule already requires independent re-check
  before any cached success is reused.

## Pinning tests

- `packages/manager` — `package_policy_changes/{document,decisions/context,replacements}.rs`
  (`WrongComparison`, `ChangedFindings`, `UnknownSubject`,
  `NonBlockingChange`), `capability_conflicts/transaction/package_lock.rs`
  (`DecisionSourceMismatch`).
- `terminal-codec` / `compilation-report` — PCC custody tests and the
  substituted-digest replays (e.g. `x86_feature_admission`
  `terminal_product_retains_exact_fma_operation_plan_and_x86_admission`).
- `component-description` — `component_verification/tests.rs` (corrupt,
  schema, subject and realization mismatches reject).
