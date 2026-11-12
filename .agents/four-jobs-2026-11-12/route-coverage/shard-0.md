# Job 1 route coverage — shard 0

- PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
- Branch: `work/route-coverage-shard-0`
- Shard rule: fixture index `i % 8 == 0`
- Fixtures: 2174 (checksum 2174)

## Program verdict rules

- Any root `rejected` → program **rejected** (judged).
- All roots `accepted` → program **accepted** (judged).
- `accepted` mixed with outside / no-entry / unreached / unjudged, no `rejected` → **partial** (not in agreement denominator).
- Otherwise **outside** (or timeout/error).

## Counts

| metric | count |
| --- | ---: |
| accepted (judged) | 37 |
| rejected (judged) | 63 |
| judged (accepted + rejected) | 100 |
| agree | 84 |
| disagree | 16 |
| partial | 195 |
| outside | 1879 |
| timeout | 0 |
| error | 0 |
| vacuous (every accepted root obligations=0 judged=0) | 108 |

Identity: agree + disagree = judged (100 = 100).
Identity: outside + partial + judged + timeout + error = 2174.

## Non-construct buckets (outside only)

| bucket | count |
| --- | ---: |
| meta:no-entry | 153 |
| meta:unreached | 119 |

## Top 10 construct refusals (outside rows only)

| construct | count |
| --- | ---: |
| Unsupported: machine service reach | 331 |
| Unsupported: parameter mutable parameter | 272 |
| Unsupported: machine generic parameters | 180 |
| Unsupported: type reference | 144 |
| Unsupported: declaration Build selection that is not a machine or a data type | 130 |
| Unsupported: type data with custody | 100 |
| Unsupported: machine attached data | 82 |
| Unsupported: type constrained | 56 |
| Unsupported: type float | 41 |
| Unsupported: declaration const | 25 |

## Top partial first-refusal constructs

| first refusal | count |
| --- | ---: |
| Unsupported: parameter mutable parameter | 40 |
| Unsupported: machine generic parameters | 33 |
| Unsupported: type reference | 21 |
| Unsupported: machine attached data | 13 |
| Unsupported: type constrained | 13 |
| outside | 12 |
| Unsupported: type data with custody | 8 |
| Unsupported: machine service reach | 6 |
| Unsupported: contract contracts on a state | 5 |
| Unsupported: expression match on a scalar subject | 4 |
| Unsupported: contract term other than a formal or literal | 4 |
| Unsupported: machine operator spelling | 3 |
| Unsupported: machine type equations, owned data or conformances | 3 |
| Unsupported: type fixed array | 3 |
| Unsupported: contract clause that is not a comparison or conjunction | 3 |

## Route accepts, current rejects (soundness candidates)

(none)

## All disagreements

- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_local_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_field_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected
- `tests/omega/pass/machines/private_division_exact_pair_exit/main.omg` tier=pass route=rejected current=accepted roots=compilation:rejected

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
python3 .agents/four-jobs-2026-11-12/route-coverage/reparse_shard_0.py
```
