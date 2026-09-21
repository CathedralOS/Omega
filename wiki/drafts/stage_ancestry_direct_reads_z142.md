# STAGE-ANCESTRY-DIRECT-READS — z142 verify+record

Board row: TASKS.md:18326 — resolved sibling stub on the settled
SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL surface. Re-verified at `74dda185a13`.

## Mined-source row

`83766d57bf` moved custody reads in
`selected-instructions-to-selected-instructions` to the retained
`optimized_target_owner` handle; staged types expose
`selected`/`register_environment`/`selections`/`budget_per_pass`/`liveness`/
`ranges`/`legality` directly; `tests/ancestry_contract.rs` pins the
contract — zero `.optimized_target()` data reads, no unsanctioned
`selected_stage()` walks.

## State at 74dda185a13 — still settled

- `grep -rn "\.optimized_target()"
  omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/`
  → zero hits (non-test source carries no reads at all).
- Witness on linux x86-64: `cargo nextest run -p
  selected-instructions-to-selected-instructions --test ancestry_contract`
  → 2/2 PASS (`staged_types_read_current_data_not_producer_ancestry`,
  `named_stage_hops_stay_at_custody_sites`).
- Surviving `live_range_stage`/`liveness_stage`/`source_legality_stage`/
  `source_segment_home_stage`/`transformation_stage` hops are the
  contract's named custody-validator inputs, not data reads.

## Verdict

Resolved — no code change. Sibling stubs on the same settled surface:
RO-S2S-ANCESTRY-WALKS, RO-STAGE-ANCESTRY-ELIMINATION,
SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION/-READS/-REMOVAL,
SELECTED-REWRITE-ANCESTRY-REMOVAL (z142 record `abc880879e`),
STAGED-ANCESTRY-ELIMINATION (z142 record `1a7872743b`).
