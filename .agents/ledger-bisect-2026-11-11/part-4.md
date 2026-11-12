# Ledger bisect part 4 (k=4, N=40)

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Prior slice (report at `741270df60f7`, markdown 226 clusters)

- Prior cluster indices: [4, 44, 84, 124, 164, 204]
- Recovered **36** member paths (incl. truncated cluster rows).

- Re-measured on PIN: **15 fixed**, **21 still broken**.

## PIN clusters (still broken, by first diagnostic)

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status | stability |
| ---: | --- | --- | --- | --- | --- | --- | --- | --- |
| 5 | pass/run rejected | cannot compare `Tree` values with `==`: structural equality is synthesized only through a named implementation | `pass/relevance/runtime_static_proof_namespace_exit` | `` |  | no first-parent good endpoint: ledger stamp and sweep baseline both still worse for witnes | propagated |  |
Members: `pass/proofs/case_call_premises`, `pass/proofs/cauchy_predicates_compile`, `pass/proofs/defined_proposition_premises`, `pass/relevance/runtime_static_proof_namespace_exit`, `pass/traits/structural_array_crash_identity_exit`

| 4 | fail wrong reason | machine `incorrect` ensures contract proof fact `result == 4` is disproved by constant arithmetic on the trans | `fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/build/helper_contract_context`, `fail/contracts/replacement_call_child_false_snapshot`, `fail/expressions/cast_result_range`, `fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected`

| 2 | pass/run rejected | generated generic application does not match its exact retained instance origin | `pass/generics/runtime_computed_nested_const_call_exit` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `pass/build/const_index_dependency_order_exit`, `pass/generics/runtime_computed_nested_const_call_exit`

| 2 | fail wrong reason | integer node `u8(300)` does not fit exact carrier `u8` | `fail/generics/const_data_argument_out_of_range` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/generics/const_data_argument_out_of_range`, `fail/generics/dependent_const_carrier_range`

| 1 | fail wrong reason | anonymous operand `25/8` is not an integer in closed const argument; type an operand before division if intege | `fail/generics/grouped_decimal_const_fraction_rejected` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/generics/grouped_decimal_const_fraction_rejected`

| 1 | fail wrong reason | cannot compare against payload-bearing case `Chain::Cons` with `==`: structural equality is synthesized only t | `fail/float/state_constructor_ieee_reflexivity` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/float/state_constructor_ieee_reflexivity`

| 1 | fail wrong reason | cannot prove requires contract for call observe from main: cells[0] in Cell::Issued in state `entry` | `fail/borrows/owned_capture_weaker_formal_qualification` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/borrows/owned_capture_weaker_formal_qualification`

| 1 | fail wrong reason | closed const argument constant expression call argument `4` does not form parameter `right` of `Float::divide` | `fail/generics/grouped_decimal_const_suffix_rejected` | `` | 2505 | bisect endpoint collapsed to PIN only (no strict good at ledger stamp); use sweep good 759 | propagated |  |
Members: `fail/generics/grouped_decimal_const_suffix_rejected`

| 1 | fail wrong reason | const parameter `Value` of `Indexed` has an ineligible index type: indices require finite structural values wi | `fail/constants/predicate_constant_generic_static_index_fence` | `` |  |  | pending |  |
Members: `fail/constants/predicate_constant_generic_static_index_fence`

| 1 | fail wrong reason | division by zero in closed const argument: the divisor is provably zero, which always traps at runtime. Remove | `fail/generics/grouped_decimal_const_zero_division_rejected` | `` |  |  | pending |  |
Members: `fail/generics/grouped_decimal_const_zero_division_rejected`

| 1 | fail wrong reason | indexed-domain selected constant has canonical type `named(name(nominal\\(package-owner\\(32:2a7c1be4c2f9cc4e1 | `fail/modules/packaged_structured_const_wrong_carrier` | `` |  |  | pending |  |
Members: `fail/modules/packaged_structured_const_wrong_carrier`

| 1 | pass/run rejected | proposition `chained` expects 2 proof-static binder argument(s), received 0 | `pass/proofs/family_carrier_relation_laws` | `` |  |  | pending |  |
Members: `pass/proofs/family_carrier_relation_laws`

## Member lines (PIN)

- `fail/borrows/owned_capture_weaker_formal_qualification` — still_broken
- `fail/build/helper_contract_context` — still_broken
- `fail/constants/predicate_constant_generic_static_index_fence` — still_broken
- `fail/contracts/replacement_call_child_false_snapshot` — still_broken
- `fail/expressions/cast_result_range` — still_broken
- `fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected` — still_broken
- `fail/float/state_constructor_ieee_reflexivity` — still_broken
- `fail/generics/const_data_argument_out_of_range` — still_broken
- `fail/generics/dependent_const_carrier_range` — still_broken
- `fail/generics/grouped_decimal_const_fraction_rejected` — still_broken
- `fail/generics/grouped_decimal_const_suffix_rejected` — still_broken
- `fail/generics/grouped_decimal_const_zero_division_rejected` — still_broken
- `fail/modules/packaged_structured_const_wrong_carrier` — still_broken
- `fail/proofs/accepted_axiom_engine_veto` — fixed_on_main
- `fail/proofs/citation_requires_bearing_rejected` — fixed_on_main
- `fail/proofs/nat_metric_false_gap_zero` — fixed_on_main
- `fail/proofs/structural_application_decreasing_false_rejected` — fixed_on_main
- `fail/traits/structural_identity_distinct_values` — fixed_on_main
- `fail/traits/structural_identity_exact_carrier` — fixed_on_main
- `fail/traits/structural_identity_rebound_reference` — fixed_on_main
- `pass/borrow/owned_capture_weaker_formal` — fixed_on_main
- `pass/build/const_index_dependency_order_exit` — still_broken
- `pass/contracts/runtime_mutable_owned_collection_poststate_exit` — fixed_on_main
- `pass/facts/target_family_contract_operand/target_body` — fixed_on_main
- `pass/generics/runtime_computed_nested_const_call_exit` — still_broken
- `pass/proofs/case_call_premises` — still_broken
- `pass/proofs/cauchy_predicates_compile` — still_broken
- `pass/proofs/citation_requires_discharged` — fixed_on_main
- `pass/proofs/defined_proposition_premises` — still_broken
- `pass/proofs/family_carrier_relation_laws` — still_broken
- `pass/proofs/lemma_requires_citation` — fixed_on_main
- `pass/proofs/nat_metric_triangle_compile` — fixed_on_main
- `pass/proofs/static_machine_result_projection` — fixed_on_main
- `pass/recast/returned_same_type_view_capture_exit` — fixed_on_main
- `pass/relevance/runtime_static_proof_namespace_exit` — still_broken
- `pass/traits/structural_array_crash_identity_exit` — still_broken

## Commands
- `mbx build -p compiler --bins --tests`; direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER`
- Host linux x86_64; `OMEGA_CORPUS_JOBS=1`
