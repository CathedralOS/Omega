# OPTIMIZER-RULE-AXIS-GATE — re-verification record

Board row: TASKS.md `OPTIMIZER-RULE-AXIS-GATE`, already resolves as "Scope
verified — re-mine of the optimizer board's PER-RULE-COVERAGE axis gate". This
draft is the re-verification record for the swarm pass; the board row itself is
uncontested-correct and needs no edit.

## What the row claims

The axis gate described by the PER-RULE-COVERAGE requirement ("every inventory
row and every rewrite reachable from a stage exercises each axis through its
stage's public entrance") is already landed and green in
`tests/architecture/optimizer_rollout/`:

- `coverage.rs` derives the expected rule set from `Optimization::ALL` plus the
  stage catalogs, reconciles name/phase/applicability/rollback per rule, and
  fails any row lacking an axis leg without a closed absent-reason.
- `inventory.rs` guards the catalog extraction: broad levels and build modes
  cannot masquerade as exact rules; catalog rows must be canonical rules of the
  catalog phase; member extraction and table termination are enforced.
- `promotion.rs` pins the promotion-record schema (exact identity, completed
  evidence, no early approval, no template/empty pending values).

## Re-verification at `c93cceb9ca` (linux x86-64)

```
cargo nextest run -p omega-architecture-test --test optimizer_rollout
```

9/9 PASS, including the board-pinned gate
`exact_rule_rollout_is_complete_and_promotion_gated` and the failure-mode leg
`the_gate_flags_every_table_failure_mode`.

## Residual ownership

The row correctly attributes the remaining work to the parent PER-RULE-COVERAGE
item, not this gate:

- missing disabled / exact-selection / identity / composition legs through
  `optimize_selected_instructions`;
- native-differential coverage for the still-uncalled `rewrites/` modules;
- the shared rule-fixture matrix harness.

Those surfaces (`optimizer_rollout`, `optimization-core`,
`selected-instructions-to-selected-instructions`, `tests/native-differential`)
are owned by sibling board items and their live claims — no independent slice
exists under this row. The row stays a re-mine pointer; this record witnesses
that the gate itself still holds at current main.
