# SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION — re-verification ledger

Board row: `TASKS.md` `**SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION.**`
(:17414) — resolved sibling stub on the settled
SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL surface (:9612). Elimination
landed: staged types expose `selected`/`register_environment`/
`selections`/`budget_per_pass`/`liveness`/`ranges`/`legality` directly,
`83766d57bf` moved custody reads to `optimized_target_owner`, and
`tests/ancestry_contract.rs` pins zero `.optimized_target()` data reads
(the surviving `liveness_stage()`/`selected_stage()` hops are
custody-validator inputs, not data reads).

## Re-verification at `74dda185a1` (linux x86-64, Zergling-126, claim `ecd08cbc`)

`cargo nextest run -p selected-instructions-to-selected-instructions
--test ancestry_contract` — 2/2 PASS:
`staged_types_read_current_data_not_producer_ancestry`,
`named_stage_hops_stay_at_custody_sites`. Matches the `74537d6125c`
witness; no independent slice exists.
