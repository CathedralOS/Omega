# Shard 4 audit supplement

## Bucket transition (c0a4894 → post-fix)

| Old | New | Count | Rule |
| --- | --- | ---: | --- |
| `discarded-valid` | `valid-route-outside` | 749 | Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). |
| `rejected-both` | `rejected-current-no-route` | 291 | Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. |
| `rejected-both` | `discarded-valid` | 197 | reclassify |
| `confirmed-hole` | `valid-route-outside` | 53 | Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge. |
| `discarded-valid` | `disagreement-valid` | 27 | Checker disagreement after strict validity reassessment. |
| `confirmed-hole` | `discarded-valid` | 3 | Hole demoted: missing spec quote, obligation, or no-trap evidence. |
| `confirmed-hole` | `known-gap-q7-mut` | 2 | Ownership/Q7 mutability gap (remove_mut / shared_instead_of_mut) per lead notes. |

**rejected-both strict-check failures after fix:** 0

## Spot checks (5 per moved flow)

### discarded-valid → valid-route-outside (749 rows)

- `tests/omega/pass/arithmetic/mixed_len_compare/main.omg` / `strip_and_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=no-entry)
- `tests/omega/pass/arithmetic/qualified_cast_in_entry_body_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/arrays/qualified_array_literal_retains_element_policy_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)
- `tests/omega/pass/asm/checked_asm_compare_one_block_exit/main.omg` / `remove_transition_guard`: Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict). (cur=accepted, route=outside)

### discarded-valid → rejected-current-no-route (0 rows)

- `tests/omega/pass/arithmetic/anonymous_rational_integer_landing/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/arithmetic/runtime_fixed_input_policy_reuse_exit/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/checked_asm_compare_one_block_exit/main.omg` / `drop_mut_let`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/checked_fence_aarch64_in_x86_body/main.omg` / `drop_requires`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)
- `tests/omega/pass/asm/scratch_hardware_flag_mix/main.omg` / `drop_mut_let`: Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection. (cur=rejected, route=∅)

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

| Kind | `confirmed-hole` | `disagreement-valid` | `discarded-valid` | `known-gap-q7-mut` | `rejected-current-no-route` | `valid-route-outside` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `delete_divisor_guard` | 0 | 15 | 37 | 0 | 0 | 773 |
| `drop_requires` | 22 | 0 | 130 | 0 | 240 | 41 |
| `remove_mut` | 0 | 0 | 10 | 160 | 15 | 162 |
| `remove_transition_guard` | 0 | 13 | 220 | 0 | 0 | 525 |
| `shared_instead_of_mut` | 0 | 0 | 293 | 2 | 36 | 49 |
| `strip_and_guard` | 0 | 16 | 40 | 0 | 0 | 521 |
| `widen_requires` | 0 | 15 | 18 | 0 | 0 | 267 |
