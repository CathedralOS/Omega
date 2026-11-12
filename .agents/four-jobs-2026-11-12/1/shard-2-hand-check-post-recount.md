# Post-recount hand-check (5 fixtures)

After the `partial` rule, only **one** row remains `route_verdict=accepted` with `current_verdict=rejected` (soundness watch). Four additional rows are former tally-disagree cases now classified `partial`.

## `tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg` (soundness-candidate)

**Verdict:** Route judged compilation `accepted` (0 obligations); current rejects UEFI entry/storage-root contract — defect outside lowered entry contract (documented open gate item).

```
program	tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg	current=rejected	current_us=2918268	root=compilation	route=accepted	route_us=260	discharge_us=80	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg	no-span	target schema `ProgramStorageApplication` exposes exactly image and initial-storage roots, but `launch` declares 0 visible parameters
```

## `tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg` (was-tally-disagree-now-partial)

**Verdict:** Helper `machine:main` accepted; defect machine `invalid` is `outside` — partial, not program accept.

```
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1417650	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1417650	root=machine:invalid	route=outside	Unsupported: contract term other than a formal or literal
program	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	current=rejected	current_us=1417650	root=machine:main	route=accepted	route_us=185	discharge_us=67	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:1 bytes 8..15	shift count in machine `invalid` ensures contract is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a defined-count po
current-diagnostic	tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg	main.omg:3 bytes 95..97	shift count in machine `invalid` state `entry` return value is provably out of range and can never execute for `u64`: exact shifts prove `count < 64` (ch5 shift-count ruling -- proof-or-policy). Constrain the count's range (a ranged type, a `requires` clause, or a dominating guard), or pick a define
```

## `tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg` (was-tally-disagree-now-partial)

**Verdict:** Defect machines `change`/`invalid` outside (mutable parameter); `main` accepted — partial.

```
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1561270	root=compilation	route=no-entry
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1561270	root=machine:change	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1561270	root=machine:invalid	route=outside	Unsupported: parameter mutable parameter
program	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	current=rejected	current_us=1561270	root=machine:main	route=accepted	route_us=205	discharge_us=72	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg	main.omg:4 bytes 148..149	exact arithmetic in machine `invalid` state `entry` return value may overflow `u32`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation). Widen with an `as` cast to a larger type, constrain the operands' range (bound a parameter with a `requires` clause, or
```

## `tests/omega/fail/asm/borrow_read_after_backward_write/main.omg` (was-tally-disagree-now-partial)

**Verdict:** Defect-related machines outside; `good`/`bad` accepted — partial.

```
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1530669	root=compilation	route=no-entry
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1530669	root=machine:read	route=outside	Unsupported: type reference
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1530669	root=machine:Main::main	route=outside	Unsupported: machine attached data
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1530669	root=machine:good	route=accepted	route_us=191	discharge_us=67	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	current=rejected	current_us=1530669	root=machine:bad	route=accepted	route_us=288	discharge_us=240	reverify_us=48	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/borrow_read_after_backward_write/main.omg	no-span	statement 6 mutates `main.cell.value` while local borrow `borrowed` is still active (borrowed at statement 1; its last use is at statement 7)
```

## `tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg` (was-tally-disagree-now-partial)

**Verdict:** Multiple machines outside; `done`/`bad` accepted — partial.

```
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=compilation	route=no-entry
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=machine:finish	route=outside	Unsupported: machine type equations, owned data or conformances
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=machine:CompletionProvider::complete	route=outside	Unsupported: type data with custody
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=machine:main	route=outside	Unsupported: machine service reach
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=machine:done	route=accepted	route_us=189	discharge_us=64	reverify_us=0	reverified=0	obligations=0	judged=0	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
program	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	current=rejected	current_us=1566220	root=machine:bad	route=accepted	route_us=214	discharge_us=185	reverify_us=9	reverified=1	obligations=1	judged=1	core_refused=0	unproved=0	refused_other=0	needed_relation=0	needed_branch=0	published_held=0	uses=0	unestablished=0	windows=0	consumed=0	closed=0	weakened_joins=0
current-diagnostic	tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg	no-span	cannot prove ensures contract for exit from finish at statement 3: proposition:fact:atomic_release_written<AtomicU64,u64>(shared word.value,ack.nonce); no exact incoming reference origin for word
```
