# Producer/checker decision-sharing audit

Historical record of the cited revisions. Superseded gates, artifact formats
and checker names below are not current requirements or work to resume.
Use [verification placement](../../../omega-rust/pipeline.md#verification-placement)
and the [current task board](../../../TASKS.md) for the maintained model.

Audit of every surface where a producer-written decision, annotation, or
recorded disposition reaches a checker or consumer that must not trust it.
Covers the mined `PRODUCER-CHECKER-*` cluster (BOUNDARY-AUDIT,
DECISION-SEPARATION, DECISION-SHARING-AUDIT, SHARING-AUDIT) — those items
name the same seam family and this ledger holds their union. Delete this
ledger once that cluster closes and a permanent spec section owns the surface
table.

Verified at revision `12dea522b2` (2026-09-20, Linux x86_64) by reading the
cited code and its existing negative tests; no new test was needed because
every reachable seam already has a pinned rejection. Rows for surfaces
whose code has since been deleted are removed. Verdicts:

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
| Component description (rosters, exports, realization rows, custody) | `verify_component` | "The description is trusted for nothing": re-decode, subject reconstructed rather than read, terminal-verifier runs under the *consumer-supplied* admission profile, every module-evident row re-derived | Re-derived |
| Admission evidence (site, evidence identity, profile decision id) | `verify_obligation` | Accepted only if the exact triple is present in the receiver's `AdmissionProfile` acceptances — profile decisions are receiver-side state, not producer claims | Bound |
| Retained-artifact replay parts (selected plans, digests, custody) | `RetainedNativeArtifact::from_replayed_parts` and sibling rejoin checks | Substituted or inconsistent producer identities fail closed ("native artifact nearest-FMA does not rejoin one exact selected provider plan"); every retained row rejoins exactly one authority row | Re-derived |

## Findings

1. **No surface converts a recorded decision into authority.** Lock-stored
   policy decisions are history: capture validates the exact
   subject/comparison, recovery validates framing and source binding, and
   the authority path always re-resolves against the *fresh* comparison
   fingerprint. A stale saved document cannot satisfy a changed comparison
   (pinned: `review/decision/document.rs` wrong-comparison and
   `ChangedFindings` tables).
2. **A recorded `reject` can never publish.** `capture_policy` preserves
   rejections as history; "merely recording one cannot permit its
   publication" (`lock/decisions/mod.rs`).
3. **The receiver selects the policy everywhere.** Package acceptance
   decisions, component-verification admission profile, and proof
   `AdmissionProfile` are all consumer-supplied; no producer annotation
   narrows what is checked.

## Residual notes (not defects)

- `HistoricalPackagePolicyDecisions.comparison` is informational history:
  recovery stores it without binding it to the sibling policy rows' provenance.
  Safe under today's read set (budget + inspection only); if a future consumer
  ever consults it for authority, it must be bound the same way the fresh
  `resolve` path binds decisions to `changes.fingerprint()`.
- A future proof-derivation store (`proof_search_cache.md`) will introduce a
  new sharing surface; its own rule already requires independent re-check
  before any cached success is reused.

## Pinning tests

- `packages/package-manager` — `review/decision/{mod,document}.rs`
  (`WrongComparison`, `ChangedFindings`, `UnknownSubject`,
  `NonBlockingChange`), `lock/` (`DecisionSourceMismatch`).
