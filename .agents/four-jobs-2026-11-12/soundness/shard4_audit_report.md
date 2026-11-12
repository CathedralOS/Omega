# Shard 4 audit supplement

## Bucket transition (c0a4894 → post-fix)

| Old | New | Count | Rule |
| --- | --- | ---: | --- |
| `discarded-valid` | `valid-route-outside` | 1195 | Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). |
| `rejected-both` | `rejected-current-route-outside` | 265 | reclassify |
| `rejected-both` | `valid-route-outside` | 197 | reclassify |
| `confirmed-hole` | `valid-route-outside` | 53 | Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. |
| `discarded-valid` | `disagreement-valid` | 27 | Checker disagreement after strict validity reassessment. |
| `discarded-valid` | `rejected-both` | 5 | Superseded rename to rejected-current-no-route (same rule). |
| `confirmed-hole` | `discarded-valid` | 3 | Hole demoted: missing spec quote, obligation, or no-trap evidence. |
| `confirmed-hole` | `known-gap-q7-mut` | 2 | Ownership/Q7 mutability gap (remove_mut / shared_instead_of_mut) per lead notes. |

**rejected-both strict-check failures after fix:** 0

## Spot checks (5 per moved flow)

### discarded-valid → valid-route-outside (1195 rows)

- `tests/omega/pass/arithmetic/fixed_array_live_length_compound_guard/main.omg` / `ref_to_mut_param`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=rejected, route=no-entry)
- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `ref_to_mut_param`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=rejected, route=no-entry)
- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `strip_and_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/qualified_cast_in_entry_body_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)

### discarded-valid → rejected-current-no-route (5 rows)

- `tests/omega/pass/domains/runtime_stored_call_inequality_addition_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/callee_saved_across_nested_calls_through_call_result_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/multi_exit_callee_saved_restoration_through_other_machine_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/signed_shift_left_bound_through_parameter_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)
- `tests/omega/pass/facts/unsigned_division_remainder_through_local_exit/main.omg` / `drop_requires`: Superseded rename to rejected-current-no-route (same rule). (cur=rejected, route=rejected)

### confirmed-hole → valid-route-outside (53 rows)

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `strip_and_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `delete_divisor_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/build/default_component_abi_extents/producer/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=outside)
- `tests/omega/pass/calls/call_local_reassign/main.omg` / `remove_transition_guard`: Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. (cur=accepted, route=no-entry)

## Kind totals (canonical)

| Kind | Count |
| --- | ---: |
| `delete_divisor_guard` | 825 |
| `drop_requires` | 433 |
| `remove_mut` | 347 |
| `remove_transition_guard` | 758 |
| `shared_instead_of_mut` | 380 |
| `strip_and_guard` | 577 |
| `widen_requires` | 300 |
| **Sum** | **3620** |

## Kind × bucket

| Kind | `confirmed-hole` | `disagreement-valid` | `discarded-valid` | `known-gap-q7-mut` | `rejected-both` | `rejected-current-route-outside` | `valid-route-outside` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `delete_divisor_guard` | 0 | 15 | 35 | 0 | 0 | 0 | 775 |
| `drop_requires` | 22 | 0 | 1 | 0 | 31 | 214 | 165 |
| `remove_mut` | 0 | 0 | 0 | 160 | 0 | 15 | 172 |
| `remove_transition_guard` | 0 | 13 | 26 | 0 | 0 | 0 | 719 |
| `shared_instead_of_mut` | 0 | 0 | 0 | 2 | 0 | 36 | 342 |
| `strip_and_guard` | 0 | 16 | 21 | 0 | 0 | 0 | 540 |
| `widen_requires` | 0 | 15 | 17 | 0 | 0 | 0 | 268 |
