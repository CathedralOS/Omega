# Agreement hand-check (shard 2)

Verifier read harness `program` lines (field `current=` vs aggregated `route=`). Patterns:

- **Route accept / current reject (fail):** entry `route=no-entry`, defect machine `route=outside` with specific construct, helper machine `route=accepted` — tally aggregate is `accepted`, so **disagree** is expected when the current checker rejects the whole package.
- **Route reject / current accept (pass):** entry judged at compilation root with unproved obligations while current checker accepted — **disagree** (route stricter on this shard slice).

## Hand-check group: disagree_route_accept_current_reject

### `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg`

```
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=machine:invalid	route=outside	Unsupported: contract term other than a formal or literal
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1384882	root=machine:main	route=accepted	route_us=296	discharge_us=90	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:1 bytes 8..15	shift count in machine `invalid` ensures contract is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a defined-count po
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:3 bytes 95..97	shift count in machine `invalid` state `entry` return value is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a define
```

### `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg`

```
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:change	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:invalid	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1371065	root=machine:main	route=accepted	route_us=165	discharge_us=51	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	main.omg:4 bytes 148..149	exact arithmetic in machine `invalid` state `entry` return value may overflow `u32`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast to a larger type, constrain the operands' range (bound a parameter with a `requires` clause, or
```

### `tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg`

```
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=machine:midpoint	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	current=rejected	current_us=1303618	root=machine:main	route=accepted	route_us=162	discharge_us=52	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg	main.omg:6 bytes 147..148	exact arithmetic in machine `midpoint` state `entry` return value may overflow `u64`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast to a larger type, constrain the operands' range (bound a parameter with a `requires` clause, o
```

### `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg`

```
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=compilation	route=no-entry
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:finish	route=outside	Unsupported: machine type equations, owned data or conformances
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:CompletionProvider::complete	route=outside	Unsupported: type data with custody
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:main	route=outside	Unsupported: machine service reach
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:done	route=accepted	route_us=167	discharge_us=54	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1325187	root=machine:bad	route=accepted	route_us=161	discharge_us=134	reverify_us=22	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	no-span	cannot prove ensures contract for exit from finish at statement 3: proposition:fact:atomic_release_written<AtomicU64,u64>(shared word.value,ack.nonce); no exact incoming reference origin for word
```

### `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg`

```
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=compilation	route=no-entry
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:read	route=outside	Unsupported: type reference
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:Main::main	route=outside	Unsupported: machine attached data
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:good	route=accepted	route_us=167	discharge_us=54	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1355004	root=machine:bad	route=accepted	route_us=178	discharge_us=149	reverify_us=13	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	no-span	statement 6 mutates `main.cell.value` while local borrow `borrowed` is still active (borrowed at statement 1; its last use is at statement 7)
```

## Hand-check group: disagree_route_reject_current_accept

### `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`

```
program	tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg	current=accepted	current_us=3293408	root=compilation	route=rejected	route_us=9973	discharge_us=9333	reverify_us=1708	reverified=8	obligations=12	judged=10	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg`

```
program	tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg	current=accepted	current_us=3147686	root=compilation	route=rejected	route_us=1322	discharge_us=737	reverify_us=205	reverified=4	obligations=5	judged=4	core_refused=0	unproved=1	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg`

```
program	tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg	current=accepted	current_us=3201758	root=compilation	route=rejected	route_us=1251	discharge_us=850	reverify_us=439	reverified=5	obligations=6	judged=4	core_refused=1	unproved=1	refused_other=0	needed_relation=0	needed_branch=2	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg`

```
program	tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg	current=accepted	current_us=3123052	root=compilation	route=rejected	route_us=9536	discharge_us=9071	reverify_us=7575	reverified=10	obligations=13	judged=10	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=1	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```

### `tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg`

```
program	tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg	current=accepted	current_us=3372018	root=compilation	route=rejected	route_us=2384	discharge_us=1714	reverify_us=1095	reverified=6	obligations=8	judged=6	core_refused=0	unproved=2	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=5	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
```
