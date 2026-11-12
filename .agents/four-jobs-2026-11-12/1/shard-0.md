# Job 1 route coverage — shard 0

- PIN: `0835b3aaf803ed97f905be42d94727ff58459263`
- Harness build commit: `0835b3aaf803ed97f905be42d94727ff58459263` (binary from this checkout)
- Branch: `work/route-coverage-shard-0`
- Shard rule: fixture index `i % 8 == 0`
- Fixtures in shard: 2174 (completed 2174)

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
timeout 300 nice -n 19 /agent/repos/omega/target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

## Counts

| metric | count |
| --- | ---: |
| judged (route accepted or rejected) | 100 |
| agreed | 84 |
| disagreed | 16 |
| outside | 2074 |
| timeout | 0 |
| error | 0 |

Program-level `route_verdict`: **accepted** only when every harness `program` root reports `route=accepted`; a lone machine root cannot accept a fixture whose compilation root is `no-entry` or `outside`.

Agreement compares program-level route to harness `current=` (check-phase accept/reject on the typed frontier), not corpus tier.

Fixtures ran **serially** (one harness process at a time; `tools/build_slot.md` test-VM rule).

## Meta buckets

| bucket | count |
| --- | ---: |
| meta:no-entry | 153 |
| meta:unreached | 119 |

## Top 10 construct refusals (`Unsupported: …`)

| construct | count |
| --- | ---: |
| Unsupported: machine service reach | 337 |
| Unsupported: parameter mutable parameter | 312 |
| Unsupported: machine generic parameters | 213 |
| Unsupported: type reference | 165 |
| Unsupported: declaration Build selection that is not a machine or a data type | 130 |
| Unsupported: type data with custody | 108 |
| Unsupported: machine attached data | 95 |
| Unsupported: type constrained | 69 |
| Unsupported: type float | 44 |
| Unsupported: contract contracts on a state | 29 |

## All construct refusals (ranked)

| construct | count |
| --- | ---: |
| Unsupported: machine service reach | 337 |
| Unsupported: parameter mutable parameter | 312 |
| Unsupported: machine generic parameters | 213 |
| Unsupported: type reference | 165 |
| Unsupported: declaration Build selection that is not a machine or a data type | 130 |
| Unsupported: type data with custody | 108 |
| Unsupported: machine attached data | 95 |
| Unsupported: type constrained | 69 |
| Unsupported: type float | 44 |
| Unsupported: contract contracts on a state | 29 |
| Unsupported: type fixed array | 27 |
| Unsupported: declaration const | 25 |
| Unsupported: expression match on a scalar subject | 23 |
| Unsupported: machine type equations, owned data or conformances | 20 |
| Unsupported: machine assembly target | 17 |
| Unsupported: type generic data | 15 |
| Unsupported: contract term other than a formal or literal | 14 |
| Unsupported: contract clause that is not a comparison or conjunction | 12 |
| Unsupported: machine suspends or blocks | 11 |
| Unsupported: type addr | 9 |
| Unsupported: machine supply mode other than a checked body | 9 |
| Unsupported: machine operator spelling | 8 |
| Unsupported: machine invokes | 8 |
| Unsupported: call compiler operation call | 7 |
| Unsupported: machine target selection | 7 |

## Route accepts, current rejects (soundness candidates)

(none)

## All disagreements

- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/saturating_integer_arithmetic_through_call_result_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_8bit_shift_right_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/signed_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/thirtytwo_arg_stack_calling_plan_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_64bit_bitwise_and_mask_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_field_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_shift_left_bound_through_other_state_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/facts/unsigned_shift_right_bound_through_other_machine_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-
- `tests/omega/pass/machines/private_division_exact_pair_exit/main.omg` tier=pass route=rejected current=accepted first_refusal=-

## `installed_arrival_*` in shard

Not in this shard (`installed_arrival_without_context` is index 950 → shard 6;
`installed_arrival_wrong_controller` is index 951 → shard 7). No timeout rows here.

## Hand-checked soundness-watch rows (lead review)

Re-ran `route_one_comparison` and verified program-level aggregation (accepted only if every root is accepted). `current=rejected` is the harness check-phase field from `check_fixture_keeping_typed_frontier`, not a build or native runtime failure.


## Hand-checked disagree rows

### `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg`
- TSV row: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit/main.omg	current=accepted	current_us=3077408	root=compilation	route=rejected	route_us=2869	discharge_us=2265	reverify_us=1159	reverified=7	obligations=8	judged=7	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
### `tests/omega/pass/machines/private_division_exact_pair_exit/main.omg`
- TSV row: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/machines/private_division_exact_pair_exit/main.omg	current=accepted	current_us=3151473	root=compilation	route=rejected	route_us=4476	discharge_us=4038	reverify_us=514	reverified=4	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
### `tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg`
- TSV row: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/signed_64bit_bitwise_or_bound_through_local_exit/main.omg	current=accepted	current_us=2978142	root=compilation	route=rejected	route_us=58819	discharge_us=58095	reverify_us=50848	reverified=8	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
### `tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg`
- TSV row: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/four_variant_enum_payload_bound_through_parameter_exit/main.omg	current=accepted	current_us=4109880	root=compilation	route=rejected	route_us=10276	discharge_us=9495	reverify_us=7989	reverified=7	obligations=8	judged=7	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=6	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
### `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg`
- TSV row: route=rejected current=accepted agreement=disagree
- `program	tests/omega/pass/facts/nine_arg_stack_calling_plan_through_call_result_exit/main.omg	current=accepted	current_us=3076755	root=compilation	route=rejected	route_us=74190	discharge_us=72990	reverify_us=65682	reverified=15	obligations=19	judged=15	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=2	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
