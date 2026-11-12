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
| Fixtures in shard | 30 |
| Originals skipped (current checker does not accept) | 8 |
| Fixtures checked with zero mutation patterns | 2 |
| Mutants attempted (generated × checked) | 35 |
| Mutant rows recorded | 35 |

Mutant total stability: `mutants_attempted` equals the sum of bucket counts below (one row per attempted mutant). Fixtures with no applicable patterns do not add rows.

## Mutant buckets (sum = mutants attempted)

| Bucket | Count |
| --- | ---: |
| `confirmed_hole` | 8 |
| `known_gap_q7_mut` | 0 |
| `discarded_both_reject` | 18 |
| `discarded_route_only` | 0 |
| `discarded_valid_mutant` | 9 |
| `discarded_no_specific_rule` | 0 |
| `discarded_timeout` | 0 |
| `mutant_run_error` | 0 |
| **Total** | **35** |

## Confirmed holes by mutation kind

| Kind | Count |
| --- | ---: |
| `drop_requires` | 1 |
| `remove_mut` | 7 |

## Kept rows

### tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `values`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg`
- Current: `accepted`; route: `no-entry`
- Route detail: route=no-entry
- Broken obligation: `28:     values[direct] = 2.0;`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `guard`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `5:     guard.count = count;`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg — drop_requires

- Bucket: `confirmed_hole`
- Mutation: delete first requires line
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `8:         true -> decimal(rest - 10, count + 1) (arithmetic on `rest` formerly guarded by `count <= 9 && embed(rest) + embed(count) * 10 <= 99`)`
- Spec (`wiki/spec/language/machines.md` § Call preconditions):

> Every machine application establishes the selected contract's `requires` under the exact argument substitution.

### tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `output`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `46:         output[output_offset] = narrow_u32_to_u16_wrapping(character.point);`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `cursor`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `23:     let observed: u64 = move_selected(snapshot(cursor, 9), reset(&mut cursor) + 11);`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/asm/checked_label_borrow_last_use_exit/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `entry_storage`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_label_borrow_last_use_exit__main.omg__remove_mut__5ef632b5dd793d32/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `30:     Main::main(&mut entry_storage);`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/atomics/placed_atomic_shared_store/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `entries`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__placed_atomic_shared_store__main.omg__remove_mut__698bd3fb4dace34b/main.omg`
- Current: `accepted`; route: `no-entry`
- Route detail: route=no-entry
- Broken obligation: `6:     entries[0] = FieldEntry { key: schema.fields[0].key, representation: FieldPlan::At { offset: 0 } };`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.

### tests/omega/pass/atomics/runtime_atomic_computed_updates_exit/main.omg — remove_mut

- Bucket: `confirmed_hole`
- Mutation: remove mut from `trace`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__atomics__runtime_atomic_computed_updates_exit__main.omg__remove_mut__b0b9c59434b0adde/main.omg`
- Current: `accepted`; route: `outside`
- Route detail: route=outside
- Broken obligation: `6:     trace.code = trace.code * 10 + digit;`
- Spec (`wiki/spec/language/expressions.md` § Assignment and explicit `&mut` / `&write` loans):

> Such a loan requires existing writable authority at the selected place; owned-local mutability requires a `mut` binding when that place is the local's owned storage.


## Hand verification (5 confirmed holes)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1400964	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=2904231	root=compilation	route=outside	Unsupported: type data with custody

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg / drop_requires

- Bucket: `confirmed_hole`
- Obligation: `8:         true -> decimal(rest - 10, count + 1) (arithmetic on `rest` formerly guarded by `count <= 9 && embed(rest) + embed(count) * 10 <= 99`)`

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
(lines differ)
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__drop_requires__3a6e19b82553794b/main.omg	current=accepted	current_us=3022048	root=compilation	route=outside	Unsupported: type constrained

```

#### Hand verification: tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3741179	root=compilation	route=outside	Unsupported: type reference

```

#### Hand verification: tests/omega/pass/asm/checked_asm_nominal_symbolic_conjunction_exit/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__asm__checked_asm_nominal_symbolic_conjunction_exit__main.omg__remove_mut__dd1ae77a62b9561c/main.omg	current=accepted	current_us=2986937	root=compilation	route=outside	Unsupported: contract term other than a formal or literal

```


## Hand verification (5 discarded drop_requires, current accepts)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / drop_requires

- Bucket: `discarded_valid_mutant`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
- // Disjunctive equality requires bound an Exact profile slot.
+ // Disjunctive equality // `side == 0 || side == 1` is the convex hull [0, 1], so `position * 2 + side`
- // `side == 0 || side == 1` is the convex hull [0, 1], so `position * 2 + side`
+ // fits i32 for position in 0..=3. The product is formed in a subordinate
- // fits i32 for position in 0..=3. The product is formed in a subordinate
+ // state, and the mirrored spelling puts the parameter on the right of `==`.
- // state, and the mirrored spelling puts the parameter on the right of `==`.
+ 
- 
+ machine slot(position: i32, side: i32) -> i32
- machine slot(position: i32, side: i32) -> i32
+ requires position >= 0 && position <= 3 && (side == 0 || side == 1)
- requires position >= 0 && position <= 3 && (side == 0 || side == 1)
+ ensures result >= 0 && result < 8
- ensures result >= 0 && result < 8
+ {
- {
+     transition { _ -> product(position, side) }
-     transition { _ -> product(position, side) }
+     machine product(position: i32, side: i32) -> i32
-     machine product(position: i32, side: i32) -> i32
+     requires position >= 0 && position <= 3 && (side == 0 || side == 1)
-     requires position >= 0 && position <= 3 && (side == 0 || side == 1)
+     {
-     {
+         position * 2 + side
-         position * 2 + side
+     }
-     }
+ }
- }
+ 
- 
+ machine slot_mirrored(position: i32, side: i32) -> i32
- machine slot_mirrored(position: i32, side: i32) -> i32
+ requires position >= 0 && position <= 3 && (0 == side || 1 == side)
- requires position >= 0 && position <= 3 && (0 == side || 1 == side)
+ ensures result >= 0 && result < 8
- ensures result >= 0 && result < 8
+ {
- {
+     side + position * 2
-     side + position * 2
+ }
- }
+ 
- 
+ machine main() {
- machine main() {
+     let mut values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
-     let mut values: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
+     let direct: i32 = slot(1, 0);
-     let direct: i32 = slot(1, 0);
+     values[direct] = 2.0;
-     values[direct] = 2.0;
+     values[slot_mirrored(3, 1)] = 7.0;
-     values[slot_mirrored(3, 1)] = 7.0;
+ }
(lines differ)
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__drop_requires__20f4386e814c5bcb/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`no-entry`
- Route detail: route=no-entry
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__drop_requires__20f4386e814c5bcb/main.omg	current=accepted	current_us=1509308	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg / drop_requires

- Bucket: `discarded_valid_mutant`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
-     requires selector <= 1
+         ensures selector <= 1
-     ensures selector <= 1
+ {
- {
+     selector = 1;
-     selector = 1;
+     [7, 9]
-     [7, 9]
+ }
- }
+ machine fill(destination: &write [u64; 4], source: [u64; 2]) {
- machine fill(destination: &write [u64; 4], source: [u64; 2]) {
+     destination[1..3] = source;
-     destination[1..3] = source;
+ }
- }
+ 
- 
+ machine Main::main(main: &mut Main) reaches Console crashes Trap {
- machine Main::main(main: &mut Main) reaches Console crashes Trap {
+     let mut values: [u64; 4] = [1, 2, 3, 4];
-     let mut values: [u64; 4] = [1, 2, 3, 4];
+     values[1..3] = [values[2], values[1]];
-     values[1..3] = [values[2], values[1]];
+     let snapshot: bool = values[0] == 1 && values[1] == 3
-     let snapshot: bool = values[0] == 1 && values[1] == 3
+         && values[2] == 2 && values[3] == 4;
-         && values[2] == 2 && values[3] == 4;
+     values[1..=2] = pair();
-     values[1..=2] = pair();
+     let called: bool = values[1] == 5 && values[2] == 6;
-     let called: bool = values[1] == 5 && values[2] == 6;
+     let source: [u64; 2] = [11, 13];
-     let source: [u64; 2] = [11, 13];
+     fill(&write values, source);
-     fill(&write values, source);
+     let copied: bool = values[0] == 1 && values[1] == 11
-     let copied: bool = values[0] == 1 && values[1] == 11
+         && values[2] == 13 && values[3] == 4 && source[0] == 11;
-         && values[2] == 13 && values[3] == 4 && source[0] == 11;
+ 
- 
+     let mut selector: u64 = 0;
-     let mut selector: u64 = 0;
+     let mut nested: [[u64; 2]; 2] = [[1, 2], [3, 4]];
-     let mut nested: [[u64; 2]; 2] = [[1, 2], [3, 4]];
+     nested[selector][..] = selecting_pair(&mut selector);
-     nested[selector][..] = selecting_pair(&mut selector);
+     let captured: bool = selector == 1 && nested[0][0] == 7
-     let captured: bool = selector == 1 && nested[0][0] == 7
+         && nested[0][1] == 9 && nested[1][0] == 3 && nested[1][1] == 4;
-         && nested[0][1] == 9 && nested[1][0] == 3 && nested[1][1] == 4;
+ 
- 
+     let mut tokens: [Token; 3] = [
-     let mut tokens: [Token; 3] = [
+         Token { value: 17, marker: 65, counter: &mut main.old_first },
-         Token { value: 17, marker: 65, counter: &mut main.old_first },
+         Token { value: 19, marker: 66, counter: &mut main.old_middle },
-         Token { value: 19, marker: 66, counter: &mut main.old_middle },
+         Token { value: 23, marker: 67, counter: &mut main.old_last }
-         Token { value: 23, marker: 67, counter: &mut main.old_last }
+     ];
-     ];
+     Console::write_byte(91);
-     Console::write_byte(91);
+     tokens[1..3] = [
-     tokens[1..3] = [
+         Token { value: tokens[2].value, marker: 68, counter: &mut main.new_middle },
-         Token { value: tokens[2].value, marker: 68, counter: &mut main.new_middle },
+         Token { value: tokens[1].value, marker: 69, counter: &mut main.new_last }
-         Token { value: tokens[1].value, marker: 69, counter: &mut main.new_last }
+     ];
-     ];
+     Console::write_byte(93);
-     Console::write_byte(93);
+     transition snapshot && called && copied && captured
-     transition snapshot && called && copied && captured
+         && tokens[0].value == 17 && tokens[1].value == 23 && tokens[2].value == 19
-         && tokens[0].value == 17 && tokens[1].value == 23 && tokens[2].value == 19
+         && main.old_middle.value == 19 && main.old_last.value == 23 {
-         && main.old_middle.value == 19 && main.old_last.value == 23 {
+         true -> done()
-         true -> done()
+         false -> bad()
-         false -> bad()
+     }
-     }
+     machine done() { Console::write_byte(10); }
-     machine done() { Console::write_byte(10); }
+     machine bad() { crash Trap; }
-     machine bad() { crash Trap; }
+ }
- }
+ 
- 
+ // The entry owns ordinary local state; selection supplies no context.
- // The entry owns ordinary local state; selection supplies no context.
+ machine entry_start() reaches Console crashes Trap {
- machine entry_start() reaches Console crashes Trap {
+     let mut entry_storage: Main;
-     let mut entry_storage: Main;
+     Main::main(&mut entry_storage);
-     Main::main(&mut entry_storage);
+ }
(lines differ)
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__drop_requires__12a9849cc4390efb/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__drop_requires__12a9849cc4390efb/main.omg	current=accepted	current_us=3637507	root=compilation	route=outside	Unsupported: machine service reach

```


## Hand verification (5 remove_mut rows)

#### Hand verification: tests/omega/pass/arithmetic/disjunctive_requires_exact_slot/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__disjunctive_requires_exact_slot__main.omg__remove_mut__221870285650b17a/main.omg	current=accepted	current_us=1404860	root=compilation	route=no-entry

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_boolean_field_shift_count_exit/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_boolean_field_shift_count_exit__main.omg__remove_mut__f309fcc21272105d/main.omg	current=accepted	current_us=3042547	root=compilation	route=outside	Unsupported: type data with custody

```

#### Hand verification: tests/omega/pass/arithmetic/runtime_captured_boolean_recurrence/main.omg / remove_mut

- Bucket: `discarded_valid_mutant`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
-     let mut condition: Condition = Condition { selected: false };
+     let condition: Condition = Condition { selected: false };
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 28 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__remove_mut__87faae966e7fddd1/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__runtime_captured_boolean_recurrence__main.omg__remove_mut__87faae966e7fddd1/main.omg	current=accepted	current_us=3122143	root=compilation	route=outside	Unsupported: type constrained

```

#### Hand verification: tests/omega/pass/arithmetic/sparse_captured_utf_path_exit/main.omg / remove_mut

- Bucket: `confirmed_hole`
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
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arithmetic__sparse_captured_utf_path_exit__main.omg__remove_mut__a1bf5fc91e003f1e/main.omg	current=accepted	current_us=3834309	root=compilation	route=outside	Unsupported: type reference

```

#### Hand verification: tests/omega/pass/arrays/runtime_completed_array_window_replacement_exit/main.omg / remove_mut

- Bucket: `discarded_valid_mutant`
- Obligation: ``

**Mutant diff (excerpt)**
```diff
-         let mut cell: i32 = byte;
+         let cell: i32 = byte;
```

**`omega --check`** exit=0 verdict=`accepted`
```
compiled 29 source file(s) from /agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__remove_mut__b9a3bdab948d8393/main.omg; wrote_output=false

```

**`route_one_comparison`** route=`outside`
- Route detail: route=outside
```
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-6/mutants/tests__omega__pass__arrays__runtime_completed_array_window_replacement_exit__main.omg__remove_mut__b9a3bdab948d8393/main.omg	current=accepted	current_us=3924242	root=compilation	route=outside	Unsupported: machine service reach

```

