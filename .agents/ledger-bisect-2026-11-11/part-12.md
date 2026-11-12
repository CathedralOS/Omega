# Ledger bisect part 12 (PIN `51e8c45c793`)

Updated: 2026-11-12 09:17 UTC

## Summary

- Prior-report slice: cluster indices `i % 40 == 12` (6 rows in table; 21 member fixtures after full member recovery).
- Fixed on main at PIN: 4; still-broken rows: 17 in 2 PIN-diagnostic clusters.

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 13 | pass/run rejected | cannot prove ensures contract for exit from whole_view at statement 0: result.len == Count | `pass/facts/as_slice_views_generic_offset_selector_exit` | `2174c0d10bb1` | 1313 | compiler chooses product providers over terminal demand, shifting provider-settlement/publication so slice whole_view en | bisected |
Members: `pass/facts/as_slice_views_generic_offset_selector_exit`, `pass/facts/as_slice_views_generic_root_exit`, `pass/facts/as_slice_views_generic_shadowed_extent_exit`, `pass/facts/as_slice_views_generic_stored_reference_selectors_exit`, `pass/facts/as_slice_views_generic_through_call_result_exit`, `pass/facts/as_slice_views_generic_through_const_name_exit`, `pass/facts/as_slice_views_generic_through_field_exit`, `pass/facts/as_slice_views_generic_through_local_exit`, `pass/facts/as_slice_views_generic_through_match_join_exit`, `pass/facts/as_slice_views_generic_through_other_machine_exit`, `pass/facts/as_slice_views_generic_through_other_state_exit`, `pass/facts/as_slice_views_generic_through_parameter_exit`, `pass/facts/as_slice_views_generic_through_target_body_exit`
Stability: deterministic (PIN exit 1 ×3, good b605888 exit 0 ×1; bisect first-bad not PIN)

| 3 | pass/run rejected | boundary requirement `Comparison::equal` … resolves to 0 selected ProviderPlans; expected exactly on | `pass/operators/crash_routes` | `60cb5e1f79dd` | 1721 | psi #1721 reshapes provider admission/selection and collection-bound checks so Comparison::equal boundary requirement no | bisected |
Members: `pass/operators/crash_routes`, `pass/providers/checked_fixed_operator_dispatch_exit`, `pass/providers/checked_fixed_operator_physical_custody`
Stability: deterministic (PIN exit 1, bebbe455 exit 0, 2174 exit 0; first-bad #1721)

## Fixed on main at PIN

- `fail/borrows/evidence_bound_record_result_keeps_loan`
- `pass/generics/runtime_inherited_composite_requirement_exit`
- `fail/proofs/result_index_authored_operator`
- `fail/providers/checked_boundary_extent_index_out_of_bounds`

## Commands

- PIN: `51e8c45c793007f211d0d9baee7de16767172444`
- host: linux x86_64; `OMEGA_CORPUS_JOBS=1`
- measure: direct `corpus_runner`, records `.agents/ledger-bisect-2026-11-11/part-12-pin.txt`
- bisect: PIN-frozen `/tmp/bisect-pin/` + `/tmp/ledger-bisect-tools/bisect_witness.sh` (skips ledger-only commits); good by newest-first-parent ancestor with exit 0
