# Soundness mutations shard 6 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Status: in progress

Lead review buckets (every mutant row belongs to exactly one bucket below).

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison
export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD
python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
```

## Fixture coverage

| Metric | Count |
| --- | ---: |
| Fixtures in shard | 1130 |
| Originals skipped | 8 |
| Fixtures checked (original accepted) | 1122 |
| Fixtures with zero generated mutants (no-mutation) | 2 |
| Fixtures with ≥1 mutant row | 15 |
| Mutant rows | 35 |
| Mean mutants per mutated fixture | 2.33 |

Reconcile: `fixtures in shard` = skipped + checked; `checked` = no-mutation + mutated fixtures; mutant row buckets sum to `mutant rows`.

## Mutant buckets (sum = mutants attempted)

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 7 |
| `known-gap-q7-mut` | 0 |
| `disagreement-valid` | 0 |
| `valid-route-outside` | 9 |
| `discarded-valid` | 1 |
| `rejected-both` | 18 |
| `timeout` | 0 |
| `failed-run` | 0 |
| `failed-run` | 0 |
| **Total** | **35** |

## Fixtures with more than one mutant row

Count: 11 fixtures.
- `tests/omega/pass/arithmetic/checked_flag_proof_signed_mask/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg`: 5 rows
- `tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg`: 2 rows
- `tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg`: 4 rows
- `tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg`: 2 rows
- `tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg`: 4 rows
- `tests/omega/pass/asm/atomic_release_disposition_pair_exit/main.omg`: 2 rows
- `tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg`: 4 rows
- `tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg`: 2 rows
- `tests/omega/pass/atomics/placed_atomic_shared_store/main.omg`: 2 rows
- `tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg`: 2 rows

## Confirmed holes by mutation kind

| Kind | Count |
| --- | ---: |
| `remove_mut` | 7 |

## Confirmed holes by accepted_by

| accepted_by | Count |
| --- | ---: |
| `current` | 7 |

## Kept rows

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `values`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `28:     values[direct] = 2.0;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `guard`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `5:     guard.count = count;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `46:         output[output_offset] = narrow_u32_to_u16_wrapping(character.point);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `cursor`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor) + 11);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `30:     Main::main(&mut entry_storage);`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/atomics/placed_atomic_shared_store/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__placed_atomic_shared_store__main.omg__remove_mut__698bd3fb4dace34b/main.omg`
- Current: `accepted`; route: `no-entry`; accepted_by: `current`
- Route detail (hint only): route=no-entry
- Broken obligation: `6:     entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `no-entry`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.

### tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg — remove_mut

- Bucket: `confirmed-hole`
- Mutation: remove mut from `trace`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_atomic_computed_updates_exit__main.omg__remove_mut__b0b9c59434b0adde/main.omg`
- Current: `accepted`; route: `outside`; accepted_by: `current`
- Route detail (hint only): route=outside
- Broken obligation: `6:     trace.code = trace.code * 10 + digit;`
- (a) Compile-time spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage. Checked compilation must establish this obligation before execution; a runtime trap or crash arm does not discharge it at check time.
- Subtype: `n/a`
- (b) Check-time vs runtime: accepting checker verdict `accepted` (route `outside`); obligation must be proved at check, not deferred to crash.
- (c) Minimal diff: see hand-verification section below.


## Hand verification (5 confirmed-hole)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / remove_mut

- Bucket: `confirmed-hole`
- Obligation: `28:     values[direct] = 2.0;`

**Mutant diff (excerpt)**
```diff
-     let mut values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
+     let values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1421617	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
- Obligation: `5:     guard.count = count;`

**Mutant diff (excerpt)**
```diff
-     let mut guard: Guard;
+     let guard: Guard;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=3065003	root=compilation	route=outside	Unsupported: type data with custody

```

#### Hand verification: tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
- Obligation: `46:         output[output_offset] = narrow_u32_to_u16_wrapping(character.point);`

**Mutant diff (excerpt)**
```diff
-     let mut output: [u16; 4096];
+     let output: [u16; 4096];
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3740230	root=compilation	route=outside	Unsupported: type reference

```

#### Hand verification: tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
- Obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor) + 11);`

**Mutant diff (excerpt)**
```diff
-     let mut cursor: u64 = 2;
+     let cursor: u64 = 2;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg	current=accepted	current_us=2953075	root=compilation	route=outside	Unsupported: contract term other than a formal or literal

```

#### Hand verification: tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg / remove_mut

- Bucket: `confirmed-hole`
- Obligation: `30:     Main::main(&mut entry_storage);`

**Mutant diff (excerpt)**
```diff
-     let mut entry_storage: Main;
+     let entry_storage: Main;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg	current=accepted	current_us=3003895	root=compilation	route=outside	Unsupported: type reference

```


## Hand verification (5 discarded-valid)

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg / drop_requires

- Bucket: `discarded-valid`
- Obligation: `8:         true -> decimal(rest - 10, count + 1) (arithmetic on `count` formerly guarded by `count <= 9 && embed(rest) + embed(count) * 10 <= 99`)`

**Mutant diff (excerpt)**
```diff
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     transition rest >= 10 {
-     transition rest >= 10 {
+         true -> decimal(rest - 10, count + 1)
-         true -> decimal(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine next(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine next(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ { count }
- { count }
+ 
- 
+ machine local(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine local(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let selected: bool = rest >= 10;
-     let selected: bool = rest >= 10;
+     let copied: bool = selected;
-     let copied: bool = selected;
+     let combined: bool = copied && copied;
-     let combined: bool = copied && copied;
+     transition combined {
-     transition combined {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine replaced(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine replaced(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let mut condition: Condition = Condition { selected: false };
-     let mut condition: Condition = Condition { selected: false };
+     condition.selected = rest >= 10;
-     condition.selected = rest >= 10;
+     transition condition.selected {
-     transition condition.selected {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine field(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine field(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let condition: Condition = Condition { selected: rest >= 10 };
-     let condition: Condition = Condition { selected: rest >= 10 };
+     let copied: Condition = condition;
-     let copied: Condition = condition;
+     transition copied.selected {
-     transition copied.selected {
+         true -> next(rest - 10, count + 1)
-         true -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine denied(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine denied(rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let below: bool = rest < 10;
-     let below: bool = rest < 10;
+     transition below {
-     transition below {
+         false -> next(rest - 10, count + 1)
-         false -> next(rest - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine historical(mut rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
- machine historical(mut rest: u64 in Trapping, count: u64 in Trapping) -> u64 in Trapping
+ requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
- requires count <= 9 && embed(rest) + embed(count) * 10 <= 99
+ {
- {
+     let observed: u64 in Trapping = rest;
-     let observed: u64 in Trapping = rest;
+     let selected: bool = rest >= 10;
-     let selected: bool = rest >= 10;
+     rest = 0;
-     rest = 0;
+     transition selected {
-     transition selected {
+         true -> next(observed - 10, count + 1)
-         true -> next(observed - 10, count + 1)
+         _ -> (count)
-         _ -> (count)
+     }
-     }
+ }
- }
+ 
- 
+ machine main() crashes Trap {
- machine main() crashes Trap {
+     let digits: u64 in Trapping = decimal(99, 0);
-     let digits: u64 in Trapping = decimal(99, 0);
+     let zero: u64 in Trapping = decimal(0, 0);
-     let zero: u64 in Trapping = decimal(0, 0);
+     let one: u64 in Trapping = decimal(10, 0);
-     let one: u64 in Trapping = decimal(10, 0);
+     let from_local: u64 in Trapping = local(49, 5);
-     let from_local: u64 in Trapping = local(49, 5);
+     let from_field: u64 in Trapping = field(29, 7);
-     let from_field: u64 in Trapping = field(29, 7);
+     let from_denial: u64 in Trapping = denied(19, 8);
-     let from_denial: u64 in Trapping = denied(19, 8);
+     let from_history: u64 in Trapping = historical(39, 6);
-     let from_history: u64 in Trapping = historical(39, 6);
+     let from_replacement: u64 in Trapping = replaced(59, 4);
-     let from_replacement: u64 in Trapping = replaced(59, 4);
+     transition digits == 9 && zero == 0 && one == 1
-     transition digits == 9 && zero == 0 && one == 1
+         && from_local == 6 && from_field == 8 && from_denial == 9
-         && from_local == 6 && from_field == 8 && from_denial == 9
+         && from_history == 7 && from_replacement == 5 {
-         && from_history == 7 && from_replacement == 5 {
+         true -> done()
-         true -> done()
+         _ -> wrong()
-         _ -> wrong()
+     }
-     }
+     machine done() {}
-     machine done() {}
+     machine wrong() { crash Trap; }
-     machine wrong() { crash Trap; }
+ }
- }
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg	current=accepted	current_us=3052701	root=compilation	route=outside	Unsupported: type constrained

```

