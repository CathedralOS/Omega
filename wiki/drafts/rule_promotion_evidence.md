# RULE-PROMOTION-EVIDENCE — re-verification ledger

Re-verified on Linux x86-64 at `ea78f0e486f3` (board tip at claim time) by
Zergling-126 under claim ticket `659cd15f`. The item re-mines
WORKSPACE-ROLLOUT's exact-rule promotion territory: six staged records in
`omega-rust/omega/representations/optimization-core/promotions/` must each
carry the promotion contract's full evidence set before `Approved status`
completes.

## Facts re-verified on this revision

- All six staged records present: `ControlFlowCleanup`, `CopyPropagation`,
  `DeadPureScalarElimination`, `GlobalValueNumbering`, `ProofCheckElision`,
  `SparseConditionalConstantPropagation` (+ `README.md`).
- Every record carries `Measurement evidence: PENDING — no versioned
  compile-time or output-quality benchmark recorded` — still gated on the
  BENCHMARKS native-realization failure, not implementable from this row.
- Every record's `Rollback evidence` names its
  `compiler/tests/no_selection_golden/rollback.rs` rejoin pin (byte-identical
  rejoin under `--disable-optimization <name>` on `HOSTED_NATIVE_TARGETS`,
  plus product-gating and empty-request custody pins) — the rejoin legs sit
  under PROMOTION-ROLLBACK-REJOIN-LEGS territory as recorded.
- Gate `exact_rule_rollout_is_complete_and_promotion_gated`
  (`tests/architecture/optimizer_rollout/mod.rs:37`) — re-run on this host:

```text
$ cargo nextest run -p omega-architecture-test --no-fail-fast -E 'test(~rollout)'
PASS optimizer_rollout exact_rule_rollout_is_complete_and_promotion_gated
```

## Verdict

No implementable slice exists under this name: measurement evidence waits on
the BENCHMARKS failure, rollback rejoin legs belong to
PROMOTION-ROLLBACK-REJOIN-LEGS, and owner approval plus the full-workspace
`mbx test` gate are product decisions. Sibling re-mine
RULE-PROMOTION-EVIDENCE-COMPLETION records the same settled surface.
