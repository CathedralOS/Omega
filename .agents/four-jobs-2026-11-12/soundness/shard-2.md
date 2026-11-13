# Soundness mutation shard 2 (run 2)

PIN `0835b3aaf803ed97f905be42d94727ff58459263` | branch `work/soundness-mutations-shard-2`

## Bucket totals

| Bucket | Count |
| --- | ---: |
| `confirmed-hole` | 0 |
| `known-gap-q7-mut` | 14 |
| `disagreement-valid` | 6 |
| `valid-route-outside` | 784 |
| `rejected-current-route-outside` | 1195 |
| `discarded-valid` | 6 |
| `rejected-both` | 23 |
| `timeout` | 0 |
| `failed-run` | 0 |
| **TSV rows** | **2028** |

## Mutation kind totals (canonical)

- `drop_requires`: **425** — generated via drop_machine_requires (header)
- `widen_requires`: **85** — generated via widen_requires_lt
- `remove_guard_transition`: **0** — no delete_first_if_guard mutator applied in run 1
- `remove_mut`: **523** — generated
- `mut_ref_instead_of_shared`: **328** — generated via ref_to_mut_ref
- `shared_instead_of_mut`: **558** — generated via mut_ref_to_ref
- `none`: **109** — drop_machine_ensures mapped here; ensures drops not soundness holes
- per-kind sum: **2028** (must equal TSV rows)

## Integrity

- Bucket sum equals TSV rows: **True**
- Per-kind sum equals TSV rows: **True**
- Missing route verdicts: **0**
- Confirmed holes with empty spec quote: **0**

## Fixture identity

`fixtures (1130) = mutated (924) + no-mutation (155) + original-not-accepted (51) + failed-run fixtures (0)`

## valid-route-outside breakdown

- `route=outside`: **714**
- `route=no-entry`: **70**
- total: **784**

## Original-not-accepted (genuine)

- `tests/omega/pass/asm/atomic_release_disposition_state_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/pass/asm/checked_asm_saved_scalar_match_exit/main.omg`: current=rejected — Unsupported: machine assembly target
- `tests/omega/pass/borrows/generic_record_slice_referent_exit/main.omg`: current=rejected — Unsupported: type generic data
- `tests/omega/pass/borrows/nested_reference_write_disjoint_facts_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/borrows/runtime_computed_view_observation_composition_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/borrows/runtime_copied_reference_conditional_structural_result_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/borrows/runtime_partial_reference_partition_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/build/component_declaration_scalar_conditions_exit/main.omg`: current=rejected — Unsupported: type data with custody
- `tests/omega/pass/build/dependency_implementation_closure/dependency/main.omg`: current=stopped — Some("access view `Access<UnusedPlacement,UnusedRegisters>`: schema data `UnusedRegisters` field `nested` has no reflected storage geometry: representation reci
- `tests/omega/pass/cases/case_call_premises/main.omg`: current=rejected — program	/agent/repos/omega/tests/omega/pass/cases/case_call_premises/main.omg	current=rejected	current_us=1469461	root=compilation	route=no-entry
current-diagno
- `tests/omega/pass/control_flow/runtime_ranked_state_cycle_owned_record_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/pass/facts/abstract_signature_call_generic_actual_through_match_join_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/facts/as_slice_views_generic_root_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/facts/as_slice_views_generic_through_other_machine_exit/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/computed_record_window_through_other_state_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/facts/crash_entry_representation_view_through_call_result_exit/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/data_where_standing_borrowed_field_array_other_machine/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/data_where_standing_nested_borrowed_field_field/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/data_where_standing_nested_borrowed_field_parameter/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/data_where_standing_shared_record_replacement/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/facts/domain_pattern_match_bound_through_field_exit/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/installed_entry_storage_through_local_exit/main.omg`: current=stopped — Some("access view `Access<RegPlacement,Registers>`: demanded premise closure checking failed: generic-data method lost its exact template")
- `tests/omega/pass/facts/match_computed_root_through_other_state_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/facts/match_reference_referent_through_parameter_exit/main.omg`: current=rejected — Unsupported: type reference
- `tests/omega/pass/facts/non_owning_reference_erasure_through_const_name_exit/main.omg`: current=rejected — Unsupported: type constrained
- `tests/omega/pass/facts/recoverable_replacement_window_through_other_machine_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/filesystem/native_canonicalize/main.omg`: current=timeout — timeout
- `tests/omega/pass/filesystem/runtime_portable_directory_names_exit/main.omg`: current=timeout — timeout
- `tests/omega/pass/float/symbolic_integer_conversion_finite_requires_exit/main.omg`: current=rejected — Unsupported: type float
- `tests/omega/pass/generics/generic_seq_consuming_map_filter/main.omg`: current=stopped — Some("generic machine `map` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived in machine `map$speci
- `tests/omega/pass/generics/open_const_float_owned_dependency/main.omg`: current=stopped — Some("constant initializer: Terminal result scalar is outside the compiler-owned vocabulary")
- `tests/omega/pass/generics/record_const_named_literal_equivalence/main.omg`: current=stopped — Some("conformance `Selected` parameter `Setting` requires a const argument, but `<structural type>` has another static category")
- `tests/omega/pass/generics/runtime_generic_carrier_equation_context_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/operators/crash_routes/main.omg`: current=rejected — Unsupported: machine attached data
- `tests/omega/pass/operators/runtime_declared_open_range_preserves_operand_snapshot_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/ownership/runtime_borrowed_match_repeated_repair_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/ownership/runtime_fixed_selector_owned_argument_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/pass/packages/runtime_attached_owner_crash_guard_scope_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/proofs/admitted_premises_long_equality_chain/main.omg`: current=rejected — program	/agent/repos/omega/tests/omega/pass/proofs/admitted_premises_long_equality_chain/main.omg	current=rejected	current_us=1304816	root=compilation	route=no-
- `tests/omega/pass/proofs/borrowed_descriptor_bound_local_result_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/proofs/defined_proposition_premises/main.omg`: current=stopped — Some("cannot compare `Point` values with `==`: structural equality is synthesized only through a named implementation -- declare `PointEquality satisfies Equata
- `tests/omega/pass/proofs/runtime_ranked_forwarding_state_recursion_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/pass/providers/runtime_provider_reference_allocation_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/pass/slices/runtime_mutable_slice_result_nested_reborrow_exit/main.omg`: current=rejected — Unsupported: parameter mutable parameter
- `tests/omega/pass/slices/runtime_stored_view_dynamic_selection_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/pass/terminal_psi/inspection_scalar_library/main.omg`: current=rejected — program	/agent/repos/omega/tests/omega/pass/terminal_psi/inspection_scalar_library/main.omg	current=rejected	current_us=1499627	root=compilation	route=no-entry

- `tests/omega/pass/traits/applied_default_lexical_selection_exit/main.omg`: current=rejected — Unsupported: declaration trait
- `tests/omega/pass/write_only/returned_destination_transport_exit/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/run/borrows/returned_descriptor_entry_snapshot/main.omg`: current=rejected — Unsupported: machine generic parameters
- `tests/omega/run/calls/outcome_specific_selected_guarantee_inputs_exit/main.omg`: current=rejected — Unsupported: machine service reach
- `tests/omega/run/ranges/borrowed_case_payload_alias/main.omg`: current=rejected — Unsupported: parameter mutable parameter

## Hand-check samples (≥10 per kind; confirmed holes or next-best accepted mutants)

### `drop_requires` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/asm/state_fact_transfers_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/borrows/nested_machine_unit_output_long_cycle_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 3. `tests/omega/pass/borrows/opaque_incoming_stored_byte_view_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 4. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 5. `tests/omega/pass/borrows/runtime_reference_projection_evaluation_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 6. `tests/omega/pass/facts/range_fact_delivery_through_local_exit/main.omg`
- **accepted_by:** `both` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 7. `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_violation_unreported`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 8. `tests/omega/pass/facts/signed_arithmetic_shift_right_through_local_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_violation_unreported`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 9. `tests/omega/pass/facts/signed_exact_addition_bound_through_call_result_exit/main.omg`
- **accepted_by:** `both` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 10. `tests/omega/pass/facts/signed_exact_multiplication_bound_through_parameter_exit/main.omg`
- **accepted_by:** `both` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** drop_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

### `widen_requires` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/asm/state_fact_transfers_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/borrow/runtime_signed_element_call_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 3. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 4. `tests/omega/pass/borrows/runtime_captured_primitive_reference_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 5. `tests/omega/pass/borrows/runtime_reference_projection_evaluation_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 6. `tests/omega/pass/cases/later_arm_requires/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 7. `tests/omega/pass/collections/borrowed_byte_array_state_writes_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 8. `tests/omega/pass/collections/cyclic_shared_plain_array_parameter_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 9. `tests/omega/pass/collections/runtime_dynamic_indexed_borrow_call_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 10. `tests/omega/pass/collections/runtime_fixed_byte_view_dynamic_state_shared_loan_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** widen_requires not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

### `remove_guard_transition`
_No retained mutants to review in this kind._

### `remove_mut` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/arithmetic/runtime_anonymous_array_stores_with_float_import_exit/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/calls/caller_structural_result_indexed_borrow_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -34,5 +34,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 3. `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -57,5 +57,5 @@
 // Execute both original machine bodies; the checks observe their authored values.
 machine main() crashes Trap {
-    let mut captured: CapturedLocal = CapturedLocal { answer: 0 };
+    let captured: CapturedLocal = CapturedLocal { answer: 0 };
     CapturedLocal::main(&mut captured);
     let mut conditional: ConditionalWrite = ConditionalWrite { cell: 0, answer: 0 };
```

#### 4. `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_match_join_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -51,5 +51,5 @@
 reaches Console
 {
-    let mut cell: i32 = byte;
+    let cell: i32 = byte;
     let written: i64 = PosixConsoleIo::write(1, &mut cell, 1);
 }
```

#### 5. `tests/omega/pass/locals/runtime_local_view_member_element_read_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -30,5 +30,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 6. `tests/omega/pass/storage/runtime_local_slice_forward_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -24,5 +24,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 7. `tests/omega/pass/text/declared_record_view_field_fill_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -31,5 +31,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 8. `tests/omega/pass/versioning/runtime_version_migration_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -45,5 +45,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 9. `tests/omega/pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -83,5 +83,5 @@
 // The entry owns ordinary local state; selection supplies no context.
 machine entry_start() reaches Console {
-    let mut entry_storage: Main;
+    let entry_storage: Main;
     Main::main(&mut entry_storage);
 }
```

#### 10. `tests/omega/pass/wire/runtime_wire_signed_extremes_roundtrip_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** demoted `mut` does not pair with a write or &mut on that binding

```diff
--- original
+++ mutant
@@ -78,5 +78,5 @@
 }
 machine ExtremesCodec::encode<const N: u64>(value: &ExtremeValues, out: &mut [u8; N], written: &mut u64) {
-    let mut scratch: [u8; N];
+    let scratch: [u8; N];
     let mut fields: [EncodedField; 4];
     transition CompactBinary::encode<ExtremeValues, ExtremesCodec::fields>(value, &mut out[..], &mut scratch[..], &mut fields[..], 10000u64) {
```

### `mut_ref_instead_of_shared` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/atomics/runtime_atomic_loop_frontier_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** &mut formed where the source only authorized shared `&`

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/borrows/explicit_lifetime_select/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
--- original
+++ mutant
@@ -1,4 +1,4 @@
 // An erased lifetime argument on borrow-carrying data selects the one
-// input retained by the result: `View<'buf>` wraps `&'buf mut i32`;
+// input retained by the result: `View<'buf>` wraps `&mut 'buf mut i32`;
 // `select<'left, 'right>` returns `View<'left>` — the second borrow
 // stays free to store, the first is frozen into the view.
```

#### 3. `tests/omega/pass/borrows/lifetime_return_borrow/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 4. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** &mut formed where the source only authorized shared `&`

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 5. `tests/omega/pass/borrows/runtime_record_byte_region_loan_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** &mut formed where the source only authorized shared `&`

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 6. `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
--- original
+++ mutant
@@ -1,5 +1,5 @@
 // The anonymous transition subject preserves the original exclusive writes and state-parameter local loan.
 // Expected exit: 0.
-// A `&mut` loan may name a plain local copied from a state parameter:
+// A `&mut mut` loan may name a plain local copied from a state parameter:
 // `fill(&mut held)` lends the local's claim — state parameters themselves
 // are read-only bindings, so the value moves through a local slot.
```

#### 7. `tests/omega/pass/facts/narrow_to_narrow_truncation_64to8_mask_bound_through_other_state_exit/main.omg`
- **accepted_by:** `both` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 8. `tests/omega/pass/facts/signed_64bit_bitwise_and_mask_bound_through_local_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_violation_unreported`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 9. `tests/omega/pass/facts/wrapping_integer_arithmetic_through_other_machine_exit/main.omg`
- **accepted_by:** `both` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 10. `tests/omega/pass/wire/runtime_wire_signed_extremes_roundtrip_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
--- original
+++ mutant
@@ -35,5 +35,5 @@
     let read: u64 = 0u64;
     let verdict: ExtremesVerdict = ExtremesVerdict::Invalid;
-    ExtremesCodec::encode(&value, &mut buffer, &mut written);
+    ExtremesCodec::encode(&mut value, &mut buffer, &mut written);
     ExtremesCodec::decode(&mut decoded, &buffer, &mut read, &mut verdict);
 
```

### `shared_instead_of_mut` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/arithmetic/compound_assign/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 3. `tests/omega/pass/arithmetic/runtime_wrapping_mutable_local_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 4. `tests/omega/pass/borrows/reference_cycle_projection_root_partition/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 5. `tests/omega/pass/borrows/runtime_forwarded_view_field_domain_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 6. `tests/omega/pass/borrows/runtime_record_byte_region_loan_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 7. `tests/omega/pass/borrows/transition_shared_borrow_result/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 8. `tests/omega/pass/build/build_domain_context/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (``): 
- **Validity:** reference change is semantically harmless (no exclusive use through the altered loan)

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 9. `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
--- original
+++ mutant
@@ -2,5 +2,5 @@
 // Expected exit: 0.
 // A `&mut` loan may name a plain local copied from a state parameter:
-// `fill(&mut held)` lends the local's claim — state parameters themselves
+// `fill(&held)` lends the local's claim — state parameters themselves
 // are read-only bindings, so the value moves through a local slot.
 data CapturedLocal { answer: u64; }
```

#### 10. `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_match_join_exit/main.omg`
- **accepted_by:** `current` **mech:** `check_time_pass`
- **Spec** (`wiki/spec/language/ownership.md`): Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of…
- **Validity:** write through shared `&` where `&mut` was required

```diff
--- original
+++ mutant
@@ -12,5 +12,5 @@
 
 boundary trait PosixConsoleIo {
-    machine write(descriptor: i32, buffer: &mut i32, count: u64) -> i64
+    machine write(descriptor: i32, buffer: &i32, count: u64) -> i64
     reaches Console
     terminates;
```

### `none` (10 samples, confirmed + reviewed accepted mutants)

#### 1. `tests/omega/pass/arithmetic/anonymous_integer_arith/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 2. `tests/omega/pass/build/terminal_calling_policy_execution/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 3. `tests/omega/pass/contracts/ensures_tail_forward/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 4. `tests/omega/pass/contracts/rankless_cycle_guarantee_holds_at_producers/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 5. `tests/omega/pass/dependent/runtime_index_forwarded_to_inferred_region_callee_exit/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 6. `tests/omega/pass/facts/atomic_bounded_fetch/call_result/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 7. `tests/omega/pass/facts/case_payload_transport_through_other_state_exit/main.omg`
- **accepted_by:** `both` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 8. `tests/omega/pass/facts/narrow_to_wide_sign_extension_through_other_machine_exit/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 9. `tests/omega/pass/facts/narrow_to_wide_zero_extension_through_parameter_exit/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```

#### 10. `tests/omega/pass/facts/saturating_integer_arithmetic_through_parameter_exit/main.omg`
- **accepted_by:** `current` **mech:** ``
- **Spec** (``): 
- **Validity:** none not classified for validity read

```diff
(mutant archived in shard-2-mutants-pruned.tar.gz)
```
