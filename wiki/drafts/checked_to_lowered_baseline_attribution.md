# CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION — re-verification ledger

Board row: `TASKS.md` `**CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION.**`
— leg completed; the member-by-member attribution lives in
`wiki/drafts/known_baseline_failures.md` §checked-trees-to-lowered-psi
(fenced to that doc's own live claims). This draft records fresh spot
readings only.

## Spot reading at `75650d2e94` (linux x86-64, Devin / w10-w10-11, claim `1807bbae`)

z70 frontier filter `cargo nextest run -p checked-trees-to-lowered-psi
--no-fail-fast -E 'test(~ranked_countdown) or
test(~ranked_u64_countdown) or
test(~effectful_discarded_call_writes_before_return) or
test(~scalar_array_source) or
test(~closed_record_projections_replay)'` — 77 run / 76 pass / 1 fail;
suite filter `-E 'test(~owned_record_return_source)'` — 9 run / 8 pass /
1 fail.

- **Scalar-return custody** suite narrowed to one red.
  `effectful_discarded_call_writes_before_return_across_fuel` still fails
  with the signature recorded since `a84ebca972`:
  `Lowering(Unsupported("composed Unit scalar call requires structural
  call custody"))` at `tests/owned_record_return_source.rs:306`. The
  other two reds the C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES row
  measured at `f44a1177ed` are now green:
  `discarded_scalar_invocation_precedes_whole_owned_return` (its stale
  `terminal_unit_effects.for_machine(..)` plan-ownership assertion holds
  again) and
  `source_replay_rejects_return_parameter_and_carrier_substitution` (no
  longer a dead negative control — `plan.structural_result` exists
  again, so its three carrier cases execute and reject as designed).
  Both recovered under `81f9624342` ("keep effectful Unit body plans
  beside claim-free affine returns"), which narrowed the
  claim-free-affine exclusion to Complete-only plans. The remaining red
  is the genuine production refusal from
  `src/unit/attached_unit/composed_control/`; ownership stays with
  C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES, whose acceptance is
  unchanged.
- **Ranked safe-point segment bounds** pair still green
  (terminal-fixed-fuel bounded-walk series).
- **`scalar_array_source::cyclic`** family still green (`891194236af`).
- **Closed-projection replay** still green (`7af30a1f839a`).

Fence map at this reading: `tests/owned_record_return_source.rs`,
`src/unit`, and `src/returns` are currently unfenced — the
C2L-RESIDUAL-FAILURE-ATTRIBUTION and STRUCTURAL-UNIT-LOWERING claims the
board rows recorded have drained. Adjacent live claims on this surface:
NEW-C2L-SUITE-ERASED-PROOF-FORMALS-COMPILE-FIX
(`tests/registered_callback_lifetime.rs`, ~13:53Z), PSIIR
(terminal-fixed-fuel `fuel_certification`, ~20:40Z), RC-REPOSITORY
(`src/scalar_graph/scalar_contracts.rs`, ~20:58Z), and
KNOWN-BASELINE-FAILURES-REFRESH (pathless, ~16:52Z). The canonical
ledger `known_baseline_failures.md` stays with its refresh/attribution
lanes. No independent slice exists under this name.

## Spot reading at `32a6a7fa33` (linux x86-64, Zergling-126, claim `7c2184d1`)

`cargo nextest run -p checked-trees-to-lowered-psi -E
'test(~owned_record_return_source) | test(~ranked_countdown_lowers) |
test(~ranked_u64_countdown_fails)'` — 12 run / 11 pass / 1 fail:

- **Ranked safe-point segment bounds** pair still GREEN —
  `ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
  and `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64`
  both pass (repaired by the terminal-fixed-fuel bounded-walk series;
  supersedes the stale `38054732a3cd` witness on the row that still
  read them red).
- **Scalar-return custody** family unchanged —
  `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`
  fails at the identical site recorded at `a84ebca972`:
  `Lowering(Unsupported("composed Unit scalar call requires structural
  call custody"))`; the other 10 owned_record_return_source members
  pass. Ownership stays with C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES.

No independent slice exists under this name; the ledger doc itself is
fenced to KNOWN-BASELINE-FAILURES-REFRESH /
LOWERED-UNIT-FAILURE-ATTRIBUTION lanes.
