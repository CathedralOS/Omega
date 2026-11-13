# worker-3 handoff, LOWERED-FORM-CHECKER Step 2 (2026-11-12/13)

## Done
- Nothing landed by this worker. No PR opened, no branch pushed.
- Item (1), bound-entry contract: ALREADY ON MAIN as #2770 (65ef86b3c7a, aquinas-bot,
  "compiler: check bound program entry signatures on the lowered route"), which landed
  while this worker was building the same thing. It rejects the four fail/build entry
  programs through one signature comparator shared with callable formation
  (lowered-discharge/src/callable_formations.rs, typed-trees/src/callable_signature_shapes.rs).
  This worker's independent version was discarded rather than landed as a second carrier.
  It is kept only as a patch against 380d1c53763 for reference:
  w3_entry_contract_superseded.patch (same scratchpad directory). Do not apply it.

## Measured (this worker's discarded version, at 380d1c53763 + patch, worker-3 slot)
- mbx clippy -p structure-elaboration -p lowered-discharge --all-targets -- -D warnings: clean.
- mbx test -p compiler --test lowered_route_{subjects,governance,messages,generics}:
  generics 11/11, governance+messages 3/3 ok; subjects 6 ok including the new four-fixture
  test, 5 still running when the run was stopped on the coordinator's CPU order (not failures,
  not results). Nothing here validates #2770.

## Not started: item (2), wire reverify_specialization into the compile
- Entry point: lowered_discharge::reverify_specialization (src/reuse.rs), exported from
  lowered-discharge/src/lib.rs; the compile's judging entry is
  lowered_discharge::judge_compilation (src/verdict.rs), called only from
  compiler/src/checked/checking/lowered_route.rs (RouteScope::judge).
- Today judge_compilation judges the closure (stage-04 generated machines as closed machines,
  by search) and each generic template's check form beside it. The wiring is: for each
  generated machine in the closure whose template (structure_elaboration::template_of) has an
  accepted generic form, decide the form's certificates with reverify_specialization instead
  of searching; lowered_route_generics.rs already exercises the carry outside the compile and
  is the model for the test.
- Read first: lowered-discharge README "Generic bodies", "Value parameters", "Type parameters",
  "Finite families" for what is and is not carried (a body reading a value parameter as an
  operand is not carried; fall back to search there, never accept).
- Main moved a lot under this area on 2026-11-12 (#2770, #2785, #2791) and the old-checker
  removal is in flight: re-read verdict.rs and lowered_route.rs on current main before editing.
- TASKS.md Step 2 on main now reads: "complete the installed continuation's logical contracts
  from its bound entry source (no ad-hoc entry rule); wire reverify_specialization ...".

## Slot
worker-3 is clean, detached at origin/main, nothing running.
