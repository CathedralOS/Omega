# Ledger bisect part 26

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`

## Slice summary

| still broken (reclustered) | 10 |
| fixed on main | 3 |

## Clusters

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | ---: | --- | --- |
| 4 | pass/run regression | terminating machine iterate: cannot prove rank range `0..=5` for the rank produced by the selected view | `pass/termination/nested_helper_record_measure_result_equation_exit` | `` |  |  | pending |

Members:
- `pass/borrows/runtime_projected_fixed_byte_array_local_exit`
- `pass/termination/nested_helper_record_measure_result_equation_exit`
- `pass/termination/rank_arrival_unobserved_array_extent_compile`
- `pass/termination/runtime_public_result_field_rank_arrivals_exit`

| 3 | pass/run regression | state `read` receives `count` without an exact retained backing for its loan: the access has no unique completed operand | `pass/calls/runtime_constructed_borrowed_payload_arguments_exit` | `` |  |  | pending |

Members:
- `pass/calls/runtime_constructed_borrowed_payload_arguments_exit`
- `pass/data/runtime_mixed_case_nested_common_fields_exit`
- `run/borrows/transition_guard_descriptor_fallthrough`

| 1 | pass/run regression | `main` has no executable body: this compiler does not lower a state of it yet (reference retirement: stored occurrences); in state `main::entry` | `pass/borrows/copied_intermediate_descriptor_snapshot_exit` | `` |  |  | pending |

Members:
- `pass/borrows/copied_intermediate_descriptor_snapshot_exit`

| 1 | pass/run regression | cannot prove requires contract for call agree from main: saved == holder.items[0] && saved == 7 (needs fact `observed == current && observed == 7` here; establi | `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit` | `` |  |  | pending |

Members:
- `pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit`

| 1 | pass/run regression | machine `clear_selected` cannot lend restricted field `value` to `entry`: the selected scalar referent's normal-return guarantees do not preserve the field's st | `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit` | `` |  |  | pending |

Members:
- `pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit`

- fixed on main: `fail/termination/nested_helper_record_measure_result_equation_not_decreasing`
- fixed on main: `fail/termination/nested_helper_record_measure_result_equation_wrong_subject`
- fixed on main: `fail/termination/rank_arrival_public_result_wrong_subject`

## Commands

- Host: linux x86_64 cloud VM
- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=8`, `MBX_SCHEDULER_CPUS=8`
- Measure: direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER`
- Bisect: `git bisect start --first-parent` + `.agents/ledger-bisect-2026-11-11/tools/bisect_witness_full.sh`
- `git fetch --unshallow origin` before bisect
