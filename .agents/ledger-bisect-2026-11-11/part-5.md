# Ledger bisect part 5

PIN: `51e8c45c793007f211d0d9baee7de16767172444`

## Prior slice (cluster indices 5, 45, 85, 125, 165, 205 on report #2359)

| prior rows | measured members | fixed at PIN | still broken at PIN |
| ---: | ---: | ---: | ---: |
| 34 (recovered; index-5 row (+22) via report batch corpus) | 34 | 23 | 11 |

At PIN these rows re-cluster into **2** still-broken diagnostics (11 rows); **23** prior-slice rows are fixed on main.

## Still-broken clusters at PIN

| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 10 | pass/run | machine `main` state `entry` statement 0: stored call has no complete statement-successor effects: an invocati | `pass/facts/selected_native_code_layout_through_other_state_exit` | `d26fa952d579` | 1909 | psi: fuse terminal calls into the caller activation (#1909) | bisected |

Members:
- `pass/calls/native_code_layout_discarded_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_const_name_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_field_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_local_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_match_join_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_other_state_exit` (broken at PIN)
- `pass/facts/selected_native_code_layout_through_target_body_exit` (broken at PIN)
- `pass/facts/source_indexed_family_alpha_positive` (broken at PIN)
- `pass/facts/source_indexed_family_recursive_index_positive` (broken at PIN)
- `pass/proofs/erased_mathematical_initializers` (broken at PIN)

| 1 | pass/run | construction of `Selected` field `value` is not proven in domain `u64::AtLeast73` in consume state `entry` at  | `pass/facts/domain_pattern_match_bound_through_field_exit` | `66ee9999011e` | 1582 | psi: query validation's expression intervals at use, not as flow facts (#1582) | bisected |

Members:
- `pass/facts/domain_pattern_match_bound_through_field_exit` (broken at PIN)

## Fixed on main at PIN (prior-slice rows)

- `fail/borrows/applied_provider_output_requires_backing`
- `fail/borrows/copied_active_case_snapshot_retains_backing`
- `fail/borrows/nested_machine_carried_live_child_rejected`
- `fail/borrows/nested_machine_unit_output_local_escape_rejected`
- `fail/build/receiver_view_writeback_local_escape`
- `fail/facts/borrowed_store_call_through_call_result_rejected`
- `fail/facts/borrowed_store_call_through_const_name_rejected`
- `fail/facts/borrowed_store_call_through_match_join_rejected`
- `fail/facts/borrowed_store_call_through_other_machine_rejected`
- `fail/facts/borrowed_store_call_through_other_state_rejected`
- `fail/facts/borrowed_store_call_through_parameter_rejected`
- `fail/facts/borrowed_store_call_through_target_body_rejected`
- `fail/facts/source_indexed_family_insufficient_consumer_bound`
- `fail/generics/extraction_foreign_case`
- `fail/host/descriptor_read_guarded_prefix_without_admitted_abi`
- `fail/wire/reflection_local_field_loan_escape`
- `pass/build/uefi_image_residence_observation`
- `pass/domains/domain_pattern_match_bound_full_width_field_exit`
- `pass/facts/target_family_callback_guarantee/target_body`
- `pass/wire/library_compact_binary_packed_reflection_exit`
- `pass/wire/runtime_wire_encode_borrowed_scalar_slice_exit`
- `pass/wire/runtime_wire_encode_interior_scalar_slice_exit`
- `pass/wire/runtime_wire_exact_array_without_count_exit`

## Prior slice rows (report indices)

- index 5: 25 rows, witness `pass/build/uefi_image_residence_observation` (22 truncated → 4 recovered)
- index 45: 4 rows, witness `pass/wire/library_compact_binary_packed_reflection_exit` (0 truncated → 4 recovered)
- index 85: 2 rows, witness `pass/domains/domain_pattern_match_bound_full_width_field_exit` (0 truncated → 2 recovered)
- index 125: 1 rows, witness `pass/facts/target_family_callback_guarantee/target_body` (0 truncated → 1 recovered)
- index 165: 1 rows, witness `fail/generics/extraction_foreign_case` (0 truncated → 1 recovered)
- index 205: 1 rows, witness `fail/borrows/nested_machine_carried_live_child_rejected` (0 truncated → 1 recovered)

## Commands

- Host: `Linux cursor 6.12.113+ #1 SMP PREEMPT_DYNAMIC Mon Nov  9 16:04:37 UTC 2026 x86_64 x86_64 x86_64 GNU/Linux`
- Jobs: `OMEGA_CORPUS_JOBS=1`, direct `corpus_runner` via `.agents/ledger-bisect-2026-11-11/run_pin_corpus.sh`
- Bisect: fixed `/tmp/ledger-bisect-corpus_outcomes.txt` + `/tmp/ledger-bisect-witness.json`; skip ledger-only commits (`corpus_outcomes.txt` / `*.md` only); good via first-parent walk
- Statement-successor good: `5c6c16d4d1867bcac99343285c5074169d7cbfbc`
- Domain-pattern narrow good: `12a1feb4aa100ab41afd8cc59f53969c2ffc4009`
