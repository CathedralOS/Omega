# Route coverage shard 6 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
Host: `cursor`
Harness: `/agent/repos/omega/target/debug/examples/route_one_comparison`

## Build and run

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null
```

Shard selection: sorted `main.omg` corpus list with 0-based index `i % 8 == 6`.
Fixtures in shard: 2174.

Execution: **one fixture per process, serial** (see `tools/build_slot.md`).

- pass/run: `timeout 300 /agent/repos/omega/target/debug/examples/route_one_comparison <fixture>`
- fail: `timeout 300 /agent/repos/omega/target/debug/examples/route_one_comparison --every-machine <fixture>`

## Totals

- judged (route accepted or rejected): 88
- route accepted: 34
- route rejected: 54
- partial: 204
- outside: 1882
- timeout: 0
- error: 0
- agree: 71
- disagree: 17
- agreement n/a (outside + partial + timeout + error): 2086
- route accepted + current rejected (watch list, not all candidates): 0

- `meta:no-entry` (in outside/partial rows): 368
- `meta:unreached` (in outside/partial rows): 140

### Timeouts

(none in this shard)

### Slow fixtures

- `tests/omega/fail/build/installed_arrival_without_context/main.omg`: 288.843s, route `outside`, current `accepted` (completed under 300s cap).

## Top 10 first-refusal constructs (outside/partial)

| construct / feature | blocks |
| --- | ---: |
| `machine service reach` | 347 |
| `parameter mutable parameter` | 269 |
| `machine generic parameters` | 179 |
| `type reference` | 137 |
| `declaration Build selection that is not a machine or a data type` | 112 |
| `type data with custody` | 100 |
| `type constrained` | 56 |
| `machine attached data` | 49 |
| `contract contracts on a state` | 33 |
| `declaration const` | 31 |
## Agreement rules

`current_verdict` is `route_one_comparison`'s typed-frontier checker (`current=`): `accepted`, `rejected` (checker diagnostics via `current-diagnostic` lines), or `stopped` (parse/type failure before the typed frontier — not a proof rejection).

`route_verdict` is **accepted** only when every program root returned `route=accepted`; **partial** when any root accepted while another root is outside, no-entry, unreached, incomplete, or otherwise unjudged; **rejected** when every root is accepted or rejected and at least one rejected.

Agree/disagree counts apply only to judged rows (`accepted` or `rejected` route verdict).

### Route accepts, current rejects (soundness watch) by tier × diagnostic

| tier | first diagnostic class | count |
| --- | --- | ---: |

### Five soundness-watch hand checks

- Only 0 watch rows in this shard after partial correction.

### Five route rejects, current accepts

- `tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg`: `program	tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg	current=accepted	current_us=2910632	root=compilation	route=rejected	route_us=3957	discharge_us=3689	reverify_us=3237	reverified=4	obligations=6	judged=4	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg`: `program	tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg	current=accepted	current_us=2967019	root=compilation	route=rejected	route_us=2208	discharge_us=1822	reverify_us=461	reverified=6	obligations=9	judged=8	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg`: `program	tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg	current=accepted	current_us=2980169	root=compilation	route=rejected	route_us=2298	discharge_us=1901	reverify_us=627	reverified=8	obligations=13	judged=10	core_refused=0	unproved=3	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg`: `program	tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg	current=accepted	current_us=3835982	root=compilation	route=rejected	route_us=9740	discharge_us=9228	reverify_us=8010	reverified=7	obligations=8	judged=7	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=6	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`
- `tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg`: `program	tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg	current=accepted	current_us=3279357	root=compilation	route=rejected	route_us=1970	discharge_us=1517	reverify_us=1022	reverified=6	obligations=8	judged=6	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=5	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0`

## All disagreements

| fixture | tier | current | route | first_refusal |
| --- | --- | --- | --- | --- |
| `tests/omega/pass/arithmetic/nonlinear_square_positive_operand_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/arithmetic/runtime_boolean_state_shift_count_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/calls/private_arrival_guarded_helper_range_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/four_variant_enum_payload_bound_through_other_machine_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/match_join_multi_variant_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/nine_arg_stack_calling_plan_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/register_stack_arg_calling_plan_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/saturating_integer_arithmetic_through_other_state_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_field_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_8bit_division_quotient_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/signed_8bit_exact_subtraction_bound_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_64bit_bitwise_or_bound_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_64bit_bitwise_xor_bound_through_parameter_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_division_remainder_through_call_result_exit/main.omg` | pass | accepted | rejected | `` |
| `tests/omega/pass/facts/unsigned_shift_right_bound_through_local_exit/main.omg` | pass | accepted | rejected | `` |
