# STAGED-ANCESTRY-ELIMINATION — verification record (2026-09-20, `7110606f46`)

Bare mined stub at `TASKS.md:10847` ("verify scope then implement"; no
`**ITEM.**` marker — claimed freeform). The row is already enumerated as a
sibling stub on the settled SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL surface
by the resolved SELECTED-OPTIMIZATION-DIRECT-READS row (:10410).

Re-verified at `7110606f46e5d444feba184f896eb327516b59c7` (linux x86-64):

- `cargo nextest run -p selected-instructions-to-selected-instructions
  --test ancestry_contract` — **2/2 PASS**
  (`staged_types_read_current_data_not_producer_ancestry`,
  `named_stage_hops_stay_at_custody_sites`).
- Zero `.optimized_target()` data reads in the crate source — only
  `optimized_target_owner()` custody owners remain.
- Exactly 4 `liveness_stage()`/`selected_stage()` hops survive, all
  custody-validator inputs: `validate_optimized_liveness_custody`
  (`analyses/liveness/staging/validation.rs:36`),
  `validate_optimized_live_range_custody`
  (`analyses/legality/{validation.rs:19,compute.rs:15}`,
  `selected_optimization/optimization_output.rs:53`) — the contract's
  sanctioned hops, not data reads.

No independent slice exists. Sibling stubs on the same settled surface per
the board: RO-S2S-ANCESTRY-WALKS, RO-STAGE-ANCESTRY-ELIMINATION,
SELECTED-OPTIMIZATION-ANCESTRY-ELIMINATION/-READS/-REMOVAL,
SELECTED-REWRITE-ANCESTRY-REMOVAL, STAGE-ANCESTRY-DIRECT-READS (live claim
zergling-182, exp 00:22Z), STAGED-ANCESTRY-ELIMINATION.

Verdict: verified re-mine of a resolved surface — coordinator should fold
the stub into the SELECTED-OPTIMIZATION-ANCESTRY-REMOVAL cluster. Record
only; no code change.
