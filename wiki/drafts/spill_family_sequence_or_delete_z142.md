# SPILL-FAMILY-SEQUENCE-OR-DELETE — scope verification (2026-09-20, `0f5ae41e7d`)

Mined stub at `TASKS.md:9416` (`**SPILL-FAMILY-SEQUENCE-OR-DELETE** — mined
candidate; verify scope then implement`). **Scope verified: sixth re-mine of
the `unsequenced_spill_stages/` disposition surface; no independent slice
exists this wave.**

## What the name refers to

`omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/`
holds 18 validated spill families that `stage_register_allocation` does not
yet call (the stage's own `lib.rs` doc header says so). The PIPELINE-OWNER-
CONSOLIDATION flag in `TASKS_OPTIMIZER.md` names the contract: sequence a
family behind the executable `assignment/runtime_spill` route (SPILL-
REALIZATION / POC-SPILL-FAMILY-SEQUENCING) or delete it with its `lib.rs`
re-exports — do not extend both.

Verified live at `0f5ae41e7d` — state unchanged since sibling verification
at `3dd805679c` (UNSEQUENCED-SPILL-STAGES-DISPOSITION row):

- all 18 families present and publicly re-exported in `lib.rs`;
- `stage_register_allocation` still does not call them;
- external consumers still exist: architecture tables
  (`optimizer_source_organization` entrances/ladders/retired_paths,
  `representation_ownership`, `layering`, `entrypoint_module_layout`) and
  the native-differential `pipeline_ownership` register-allocation stage
  tests (`stack_slot_coloring.rs` etc.) — so deletion is not removal of
  dead code but a coordinated consumer migration, and families form an
  internal dependency chain requiring leaf-consumer-first order.

## Why no slice is workable this wave

The whole territory is fenced by live claims at verification time
(18:36Z):

- `selected-instructions-to-register-homes` wholesale →
  **POC-SPILL-FAMILY-SEQUENCING** (Jarod / swarm-w9-poc-spill-sequencing,
  expires 2026-09-20T22:30Z) — the sequencing owner;
- `tests/architecture/optimizer_source_organization` →
  **ORPHAN-STAGE-OUTPUT-AUDIT** (expires 2026-09-20T22:30Z);
- `tests/native-differential/tests/pipeline_ownership` →
  **STRUCTURAL-UNIT-CALL-GRAPH-JOINS** (expires 2026-09-20T20:13Z).

## Outcome

No code change made; the residual stays on POC-SPILL-FAMILY-SEQUENCING /
SPILL-REALIZATION. Coordinator may collapse this stub into the
UNSEQUENCED-SPILL-STAGES-DISPOSITION cluster — sibling re-mine names on the
same directory: UNSEQUENCED-SPILL-DISPOSITION, UNSEQUENCED-SPILL-FAMILY-
DISPOSITION, UNSEQUENCED-SPILL-STAGE-DISPOSITION, UNSEQUENCED-SPILL-STAGE-
TRIAGE, UNSEQUENCED-SPILL-STAGES-SEQUENCE-OR-DELETE, PIPELINE-SPILL-FAMILY-
ORPHANS, POC-SPILL-FAMILY-DISPOSITION, POC-SPILL-FAMILY-SEQUENCING.
