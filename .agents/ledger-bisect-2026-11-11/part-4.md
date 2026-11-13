# Ledger bisect part 4 (k=4, N=40)

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

Witness stop: exit 0 is the PIN-ledger outcome, exit 1 is any other outcome that ran.
Witness class: exit 1 is the PIN diagnostic class, exit 0 is any other outcome that ran.
Exit 125 is a build or run failure only. Ledger-only commits are not edges.
Find-good is an exponential first-parent walk. Bisect is `--first-parent` between that 0 and the nearest sampled 1.

## Slice reconciliation

- Slice fixtures: **36**
- Fixed (stop = 0 on the saved PIN observation): **13**
- Broken (stop = 1 on the saved PIN observation): **23**
- Unclassified: **0**
- Fixed + broken = 36 (slice 36)
- Reconciliation: **INCOMPLETE**

Broken rows not yet owned by a finished cluster:
- `fail/borrows/owned_capture_weaker_formal_qualification`
- `fail/build/helper_contract_context`
- `fail/constants/predicate_constant_generic_static_index_fence`
- `fail/contracts/replacement_call_child_false_snapshot`
- `fail/expressions/cast_result_range`
- `fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected`
- `fail/float/state_constructor_ieee_reflexivity`
- `fail/generics/const_data_argument_out_of_range`
- `fail/generics/dependent_const_carrier_range`
- `fail/generics/grouped_decimal_const_fraction_rejected`
- `fail/generics/grouped_decimal_const_suffix_rejected`
- `fail/generics/grouped_decimal_const_zero_division_rejected`
- `fail/modules/packaged_structured_const_wrong_carrier`
- `pass/borrow/owned_capture_weaker_formal`
- `pass/build/const_index_dependency_order_exit`
- `pass/contracts/runtime_mutable_owned_collection_poststate_exit`
- `pass/generics/runtime_computed_nested_const_call_exit`
- `pass/proofs/case_call_premises`
- `pass/proofs/cauchy_predicates_compile`
- `pass/proofs/defined_proposition_premises`
- `pass/proofs/family_carrier_relation_laws`
- `pass/relevance/runtime_static_proof_namespace_exit`
- `pass/traits/structural_array_crash_identity_exit`

## Clusters

## Commands

- Fresh `CARGO_TARGET_DIR=/tmp/bisect-target-<sha>` per commit, one `mbx build` at a time.
- `python3 .agents/ledger-bisect-2026-11-11/run_cluster.py --all`
