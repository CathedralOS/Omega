# Shard 4 baseline audit (originals)

Fixture original `omega --check` at PIN returned a normal verdict (rejected), not a harness failure. Runner skips mutation when original is not accepted.

Mutant or harness could not produce a checker verdict (timeout, crash, I/O). Does not include originals that current rejects with a diagnostic.

Quick re-check (20s) timed out but shard runner recorded `current=rejected` at PIN with 90s budget — treated as slow reject, not harness failed-run.

**Original-not-accepted total: 230** (TSV: **230** `current=rejected`; **0** fixture-level failed-run).

| Category | Count | Example fixtures |
| --- | ---: | --- |
| `current-rejected-proof` | 23 | `tests/omega/pass/borrows/qualified_owner_state_loan_exit/main.omg`, `tests/omega/pass/borrows/incoming_reference_slot_replacement_exit/main.omg`, `tests/omega/pass/borrows/runtime_uniform_projected_reference_selection_exit/main.omg` |
| `current-rejected-signature` | 4 | `tests/omega/pass/facts/recoverable_replacement_window_through_parameter_exit/main.omg`, `tests/omega/pass/providers/inline_provider_owner_lifetime_telescope_unapplied/main.omg`, `tests/omega/pass/traits/implementation_value_own_const_identity/main.omg` |
| `current-rejected-other` | 11 | `tests/omega/pass/build/conditional_root_binding/main.omg`, `tests/omega/pass/calls/native_code_layout_discarded_exit/main.omg`, `tests/omega/pass/facts/installed_entry_storage_through_other_machine_exit/main.omg` |
| `current-rejected-slow` | 192 | `tests/omega/pass/arithmetic/saturating_signed_divide_min_by_neg_one/main.omg`, `tests/omega/pass/arithmetic/runtime_integer_casts_exit/main.omg`, `tests/omega/pass/arithmetic/const_fold_unsigned_landed_ops_exit/main.omg` |
| `current-stopped` | 0 | — |
| `harness-timeout` | 0 | — |
| `harness-error` | 0 | — |
| `other` | 0 | — |

- Runner excludes originals-not-accepted from mutation: **True**
- Accepted originals without mutant dirs: **18**

  - `tests/omega/pass/arithmetic/checked_flag_proof_saturating_cast_fits/main.omg`
  - `tests/omega/pass/build/build_helper_call/main.omg`
  - `tests/omega/pass/build/captured_source_generated_constant/main.omg`
  - `tests/omega/pass/build/executed_dependency_source_library/main.omg`
  - `tests/omega/pass/build/provider_package_default_exit/reading_provider/main.omg`
  - `tests/omega/pass/build/selected_const_array_length_exit/main.omg`
  - `tests/omega/pass/components/independent_same_spelling_packages/api/main.omg`
  - `tests/omega/pass/contracts/header_blocks_before_body/main.omg`
  - `tests/omega/pass/core/content_conservation_constructed_argument/main.omg`
  - `tests/omega/pass/expressions/owned_match_mixed_values/main.omg`
  - `tests/omega/pass/generics/runtime_index_carrier_named_static/main.omg`
  - `tests/omega/pass/modules/machine_constant_initializers/main.omg`
  - `tests/omega/pass/modules/nominal_constant_bodies/main.omg`
  - `tests/omega/pass/modules/packaged_structured_const_declared_carrier_exit/dependency/main.omg`
  - `tests/omega/pass/modules/qualified_case_membership/main.omg`
  - `tests/omega/pass/modules/selected_constant_nominal_alias/main.omg`
  - `tests/omega/pass/providers/named_boundary_applied_issuance/main.omg`
  - `tests/omega/pass/terminal_psi/unsupported_optimizer_component/main.omg`
