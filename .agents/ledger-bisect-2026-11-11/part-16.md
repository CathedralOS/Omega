# Ledger bisect part 16 (k=16, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## PIN clusters (still-broken regrouped by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 1 | pass/run rejected | `X86Repair::repair` has no executable body: this compiler does not lower a statement of it yet (statement sequence: assi | `pass/facts/recoverable_replacement_window_through_target_body_exit` | `22f215eb3e7d` | 2078 | psi: preserve common fields when matching case values (#2078) — omega-rust/psi/compilation/pipeline/structure-elaboration/, source/library/ | bisected |
- `pass/facts/recoverable_replacement_window_through_target_body_exit`

| 1 | pass/run rejected | statement 8 mutates `first.value` while local borrow `first_input` is still active (borrowed at statement 4; its last us | `pass/borrows/runtime_selected_reference_record_helper_exit` | `` |  |  | pending |
- `pass/borrows/runtime_selected_reference_record_helper_exit`

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "Policy::identity", reason: "attached Un | `pass/providers/checked_operator_declaration_without_application` | `` |  |  | pending |
- `pass/providers/checked_operator_declaration_without_application`

## Fixed on main (within slice clusters)

- `fail/borrows/owned_capture_weaker_formal_qualification`
- `fail/traits/dyn_finite_family_narrowed_provider`
- `pass/atomics/runtime_stored_reference_atomic_events_exit`
- `pass/borrow/owned_capture_weaker_formal`
- `pass/dependent/state_arrival_byte_predicate_guarded_compile`
- `pass/facts/write_only_indexed_reborrow_through_call_result_exit`
- `pass/facts/write_only_indexed_reborrow_through_const_name_exit`
- `pass/facts/write_only_indexed_reborrow_through_field_exit`
- `pass/facts/write_only_indexed_reborrow_through_local_exit`
- `pass/facts/write_only_indexed_reborrow_through_match_join_exit`
- `pass/recast/runtime_aggregate_slice_representation_recast_exit`
- `pass/recast/runtime_inline_shared_slice_record_read_exit`
- `pass/slices/saved_byte_predicate_fields_disjoint_exit`
- `pass/text/runtime_owned_string_byte_view_exit`

## Prior-report slice clusters (index % 40 == 16)

- Fixed rows: 14; still broken at PIN: 3

## Commands

```bash
git fetch --unshallow origin
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler --test corpus_runner --no-run < /dev/null
# OMEGA_CORPUS_FIXTURE_FILTER + OMEGA_CORPUS_RECORDS direct corpus_runner (17 fixtures)
.agents/ledger-bisect-2026-11-11/tools/run_bisect_pin.sh <witness> <out.json>
```
- Host: linux x86_64, jobs: OMEGA_CORPUS_JOBS=1
- Updated: 2026-11-12 08:30 UTC
