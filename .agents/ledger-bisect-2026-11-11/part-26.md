# Ledger bisect part 26

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Slice summary

| still broken (rows attributed below) | 10 |
| fixed on main (negatives) | 3 |

## Per-row attribution (10 broken fixtures)

| fixture | cluster | first-bad (40) | parent (40) | good anchor (40) | PR | edge verify parent×3 | edge verify bad×3 | mechanism |
| --- | ---: | --- | --- | --- | ---: | --- | --- | --- |
| `pass/borrows/runtime_projected_fixed_byte_array_local_exit` | 0 | `` | `` | `` |  |  |  | see work/verify_c0.json |
| `pass/termination/nested_helper_record_measure_result_equation_exit` | 0 | `` | `` | `` |  |  |  | see work/verify_c0.json |
| `pass/termination/rank_arrival_unobserved_array_extent_compile` | 0 | `` | `` | `` |  |  |  | see work/verify_c0.json |
| `pass/termination/runtime_public_result_field_rank_arrivals_exit` | 0 | `` | `` | `` |  |  |  | see work/verify_c0.json |
| `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` | 1 | `` | `` | `` |  |  |  | see work/verify_c1.json |
| `pass/data/runtime_mixed_case_nested_common_fields_exit` | 1 | `` | `` | `` |  |  |  | see work/verify_c1.json |
| `run/borrows/transition_guard_descriptor_fallthrough` | 1 | `` | `` | `` |  |  |  | see work/verify_c1.json |
| `pass/borrows/copied_intermediate_descriptor_snapshot_exit` | 2 | `` | `` | `` |  |  |  | see work/verify_c2.json |
| `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit` | 3 | `` | `` | `` |  |  |  | see work/verify_c3.json |
| `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit` | 4 | `` | `` | `` |  |  |  | see work/verify_c4.json |

## Cluster witnesses (summary)

### Cluster 0 witness `pass/termination/nested_helper_record_measure_result_equation_exit` (4 rows)
### Cluster 1 witness `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` (3 rows)
### Cluster 2 witness `pass/borrows/copied_intermediate_descriptor_snapshot_exit` (1 rows)
### Cluster 3 witness `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit` (1 rows)
### Cluster 4 witness `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit` (1 rows)
- fixed on main: `fail/termination/nested_helper_record_measure_result_equation_not_decreasing`
- fixed on main: `fail/termination/nested_helper_record_measure_result_equation_wrong_subject`
- fixed on main: `fail/termination/rank_arrival_public_result_wrong_subject`
