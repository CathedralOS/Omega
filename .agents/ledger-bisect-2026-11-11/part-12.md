# Ledger bisect part 12 (PIN `51e8c45c793`)

Updated: 2026-11-12 16:47 UTC

## Summary

- Prior-report slice: cluster indices `i % 40 == 12` (6 rows in table; 21 member fixtures after full member recovery).
- Fixed on main at PIN: 4; still-broken rows: 17 in 3 PIN-diagnostic clusters.
- Gap closure: witness 3× stability, full membership at first-bad/first-parent, quoted mechanisms, cluster-3 parent confirmed.

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 13 | pass/run rejected | cannot prove ensures contract for exit from whole_view at statement 0: result.len == Count | `pass/facts/as_slice_views_generic_offset_selector_exit` | `2174c0d10bb1` | 1313 | mechanism not established for the pinned `whole_view` ensures diagnostic: `2174c0d..bebbe455` changes only omega `provider_settlement.rs::se | bisected |
Members: `pass/facts/as_slice_views_generic_offset_selector_exit`, `pass/facts/as_slice_views_generic_root_exit`, `pass/facts/as_slice_views_generic_shadowed_extent_exit`, `pass/facts/as_slice_views_generic_stored_reference_selectors_exit`, `pass/facts/as_slice_views_generic_through_call_result_exit`, `pass/facts/as_slice_views_generic_through_const_name_exit`, `pass/facts/as_slice_views_generic_through_field_exit`, `pass/facts/as_slice_views_generic_through_local_exit`, `pass/facts/as_slice_views_generic_through_match_join_exit`, `pass/facts/as_slice_views_generic_through_other_machine_exit`, `pass/facts/as_slice_views_generic_through_other_state_exit`, `pass/facts/as_slice_views_generic_through_parameter_exit`, `pass/facts/as_slice_views_generic_through_target_body_exit`
Stability: first-bad `2174c0d10bb1` witness exit ×3 = [1, 1, 1]; first-parent `bebbe455e751` witness exit ×3 = [0, 0, 0]
Membership @ first-bad: `pass/facts/as_slice_views_generic_offset_selector_exit`→1, `pass/facts/as_slice_views_generic_root_exit`→1, `pass/facts/as_slice_views_generic_shadowed_extent_exit`→1, `pass/facts/as_slice_views_generic_stored_reference_selectors_exit`→1, `pass/facts/as_slice_views_generic_through_call_result_exit`→1, `pass/facts/as_slice_views_generic_through_const_name_exit`→1, `pass/facts/as_slice_views_generic_through_field_exit`→1, `pass/facts/as_slice_views_generic_through_local_exit`→1, `pass/facts/as_slice_views_generic_through_match_join_exit`→1, `pass/facts/as_slice_views_generic_through_other_machine_exit`→1, `pass/facts/as_slice_views_generic_through_other_state_exit`→1, `pass/facts/as_slice_views_generic_through_parameter_exit`→1, `pass/facts/as_slice_views_generic_through_target_body_exit`→1
Membership @ first-parent: `pass/facts/as_slice_views_generic_offset_selector_exit`→0, `pass/facts/as_slice_views_generic_root_exit`→0, `pass/facts/as_slice_views_generic_shadowed_extent_exit`→0, `pass/facts/as_slice_views_generic_stored_reference_selectors_exit`→0, `pass/facts/as_slice_views_generic_through_call_result_exit`→0, `pass/facts/as_slice_views_generic_through_const_name_exit`→0, `pass/facts/as_slice_views_generic_through_field_exit`→0, `pass/facts/as_slice_views_generic_through_local_exit`→0, `pass/facts/as_slice_views_generic_through_match_join_exit`→0, `pass/facts/as_slice_views_generic_through_other_machine_exit`→0, `pass/facts/as_slice_views_generic_through_other_state_exit`→0, `pass/facts/as_slice_views_generic_through_parameter_exit`→0, `pass/facts/as_slice_views_generic_through_target_body_exit`→0
Split candidates: 0 (none — cluster kept)
Mechanism detail: mechanism not established for the pinned `whole_view` ensures diagnostic: `2174c0d..bebbe455` changes only omega `provider_settlement.rs::settle_checked_providers` (replace terminal-demand `demanded_provider_plans`/`select_derived_provider_plans` with product-scoped settlement via `ProviderPlanningContext::Product` + `selected_provider_publication`); no `omega-rust/psi/` diff in that commit.

| 3 | pass/run rejected | boundary requirement `Comparison::equal` … resolves to 0 selected ProviderPlans; expected exactl | `pass/operators/crash_routes` | `60cb5e1f79dd` | 1721 | `omega-rust/psi/foundation/language-semantics/src/declaration_selection/provider_choices/selection.rs` (build-keyed candidate resolution): d | bisected |
Members: `pass/operators/crash_routes`, `pass/providers/checked_fixed_operator_dispatch_exit`, `pass/providers/checked_fixed_operator_physical_custody`
Stability: first-bad `60cb5e1f79dd` witness exit ×3 = [1, 1, 1]; first-parent `4d6278c81b83` witness exit ×3 = [0, 0, 0]
Membership @ first-bad: `pass/operators/crash_routes`→1, `pass/providers/checked_fixed_operator_dispatch_exit`→1, `pass/providers/checked_fixed_operator_physical_custody`→1
Membership @ first-parent: `pass/operators/crash_routes`→0, `pass/providers/checked_fixed_operator_dispatch_exit`→0, `pass/providers/checked_fixed_operator_physical_custody`→0
Split candidates: 0 (none — cluster kept)
Mechanism detail: `omega-rust/psi/foundation/language-semantics/src/declaration_selection/provider_choices/selection.rs` (build-keyed candidate resolution): drop fallback from full `covering` to `keyed` when `keyed` is empty — `match keyed.as_slice()` hits `[]` and emits no unique covering `Choice`, yielding 0 selected ProviderPlans for `Comparison::equal` boundaries.

| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "binding_subject | `pass/ownership/runtime_match_projected_subject_cleanup_exit` | `7198f568763c` | 1818 | `omega-rust/psi/compilation/validation/src/proof_contracts/contract_entailment/structural_judgment/normal_returns/match_captures.rs` — `comp | bisected |
Members: `pass/ownership/runtime_match_projected_subject_cleanup_exit`
Stability: first-bad `7198f568763c` witness exit ×3 = [1, 1, 1]; first-parent `5c85123a85d4` witness exit ×3 = [0, 0, 0]
Membership @ first-bad: `pass/ownership/runtime_match_projected_subject_cleanup_exit`→1
Membership @ first-parent: `pass/ownership/runtime_match_projected_subject_cleanup_exit`→0
Split candidates: 0 (none — cluster kept)
Mechanism detail: `omega-rust/psi/compilation/validation/src/proof_contracts/contract_entailment/structural_judgment/normal_returns/match_captures.rs` — `complete_match_subject` / `complete_declared_match_subject` / `retain_match_subject` split how owned vs declared match subjects are completed and snapshotted after `SavedSubject` retirement; at `7198f568` the witness regresses to Terminal Psi lowering `InvalidUnitMachinePlan { machine: "binding_subject", reason: "selection survivor requires an exact emitted operation-result frontier" }` (lowering site unchanged in commit).

## Cluster 3 first parent (`5c85123a85d4`)

- `git log -1 --format=%P 7198f568763c6dae7d02668838a1616c631c70aa` → parent `5c85123a85d4e9d43bf771ff9db4f09a053995ee` (docs-only commit on first-parent chain).
- Files changed in `5c85123a85d4` (no compiler sources): `TASKS.md`, `wiki/drafts/designs/invariant_repair_facts.md`, `wiki/language_guide/chapter_11_invariant_windows.md`, `wiki/spec/language/dependent_values.md`, `wiki/spec/language/domains.md`, `wiki/spec/language/state_contracts.md`.

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
- membership: `/tmp/ledger-bisect-tools/validate_membership.sh` + `fixture_test.py` (exit 0 good / 1 PIN-class bad)
