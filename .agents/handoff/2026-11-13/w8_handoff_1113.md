# worker-8 handoff: LOWERED-FORM-CHECKER Step 3, data-qualified machines

## Done
PR #2791 (`work/psi-lowered-qualified-machines`): the lowered route reads `machine Data::name(...)`
as an ordinary static machine and `Data::name(args)` as a direct call (expression call, call
statement, transition target). A subject `data X {}` that declares only a name is passed over.
Files: `structure-elaboration/src/read/{machine,calls,transitions,subjects}.rs`, new test
`compiler/tests/lowered_route_qualified.rs`, `TASKS.md` (lane clause removed).
Still refused: a machine under a generic data type or one applied at type arguments
(`attached generic data`, `attached data applied at type arguments`); any value receiver.

## Measured
- Before (audit #2745, 2026-11-12, not re-run): `machine attached data` was the first refusal in
  693 programs (616 outside, 77 partial); 337 of them bind an entry, 360 do not.
- After, 12 entry-bound programs probed with `route_one_comparison`: 5 accepted by the route
  (`pass/calls/{machine_self_call_recursion_compile,attached_terminal_machine_call_exit,state_and_machine_names_exit,attached_context_terminal_machine_call_exit}`,
  `run/calls/qualified_atomic_operation_names`), 1 fail rejected at the same site
  (`fail/calls/free_machine_named_transition_rejected`), 5 next stop at `type data with custody`
  (non-`[copy]` data, #2765's lane), 1 at `type slice`.
- No count over all 693: one fixture check took 2 to 8 minutes on the shared machine.
- `cargo xtask stage-coverage` on the newly judged programs: NOT run.

## Exact next steps
1. With the old checker being removed: `fail/calls/free_machine_named_transition_rejected` pins
   ``value call to `scan` violates required fact `value == 41` ``. The route's message is
   "cannot prove requires contract for call scan from Main::main: 40 == 41 ..." and does not
   contain it; today the row is green only because the old checker's diagnostics follow. The
   route's call-requirement message (lowered-discharge, `UnprovedObligation::message`) must
   state the callee's clause (`value == 41`) for that row to stay green. Same gap is likely
   for other call-`requires` fail rows.
2. `lowered_route_qualified.rs` uses `check_fixture_keeping_typed_frontier` and asserts the
   current checker's accept/reject; whoever removes the old stages must drop those two asserts.
3. Re-measure the 337 entry-bound programs once #2765 (affine data) lands: most are blocked on
   `type data with custody`. List: scratchpad `w8/entry_list.txt`; runner `w8/measure.sh`.
4. The 360 programs with no bound entry need a module without an entry (`elaborate` refuses
   "no bound entry"); that is its own item, not a construct lane.
5. Two neighbour rows moved `rejected -> rejected` in the corpus ring and were not compared at
   base: `fail/calls/qualified_literal_return_invalid`,
   `fail/calls/recursive_terminal_block_missing_acknowledgement` (pinned fragments matched).
