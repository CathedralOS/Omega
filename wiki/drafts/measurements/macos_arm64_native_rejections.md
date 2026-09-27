# macOS arm64 native rejections, attributed

Measured 2026-09-27 at `a9478e720fc` on macOS AArch64 (CommandLineTools
`MacOSX15.5.sdk`, pinned `nightly-2026-09-04`, `SDKROOT` unset), by
`python3 tools/corpus_gate.py --native`. The roster itself is
`tests/omega/corpus_native_macos_arm64.txt`; this document says what its
red rows MEAN, which the roster cannot.

Run shape: **724 fixtures — 287 checked, 187 built, 250 rejected. 137
executed, 105 exiting 70**, none with stdout differing from expected.

Six rows carry `exit:signal signal:5`. All six are the `*trapping*` /
`*_traps*` fixtures, where the trap is the asserted behavior — 
`pass/arithmetic/runtime_trapping_overflow_traps` states that the emitted
overflow check must execute the target trap. **They are passes.** Do not
read the roster as six native crashes.

## The headline: most native red is not native

| cause | count | reaches code generation? |
| --- | --- | --- |
| unit-plan omission | 144 | no — Psi declines first |
| native-artifact Terminal production | 34 | yes |
| provider absent on this host | 19 | no — host gating, not a defect |
| canonical abstract optimization | 6 | yes |
| terminal-authority policy / closure | 6 | yes |
| entry `Main` provisioning | 4 | no |
| nested value call | 4 | no — an explicitly rejected feature |
| native artifact lowering | 3 | yes |
| target lowering | 3 | yes |
| float operator selection / unimplemented | 6 | no — OPERATOR-BOUNDARY-CALLS |
| implicit store conversion | 3 | no — authoring, spell the `as` |
| remainder (singletons) | ~18 | mixed |

**58% of the native leg's red never reaches native code generation.** Those
144 are the same check-time omission family already being worked from the
sample tree, and repairing them is not native work. The genuinely
native-side population is roughly **50** rows: Terminal production,
abstract optimization, authority closure, artifact and target lowering.
Size the native matrix against that number, not against 250.

## Routing the 144 omissions

The largest phase is dominated by two shapes already pinned, so it is not a
new investigation. `pass/recast/runtime_bool_representation_recast_exit`
spells `let flag_bits: &u8 = &self.flag as &u8;` — a shared borrow of a
`self` place AND a reference recast, two of the three independently
sufficient triggers recorded under **REFERENCE-LOCAL-BINDING-ROUTES**. The
seven `recast/` rows in that phase route there; the seven `slices/` rows in
it bind references too and are the likely same owner, unmeasured.

Phases whose owner is already filed:

- `terminator: conditional successors: guard expression` — the float-operator
  and mixed-carrier comparison halves, both owned (OPERATOR-BOUNDARY-CALLS
  and `tests/omega/pending/expressions/mixed_integer_guard_comparison`).
- `terminator: conditional successors: parameter transfer` — adjacent to
  **COMPOSITE-TRANSITION-ARGUMENT** and **VALUE-CALL-TRANSITION-ARGUMENT**;
  `systems/vending_machine` needed both, so confirm which applies per row.
- `statement sequence: local data: scalar local: pure initializer` —
  **VALUE-CALL-SUBEXPRESSION** and the composite-local family.

Phases with NO filed owner, ranked — these are where new diagnosis pays:

- `scalar field store sequence: write frame agreement` (8)
- `structural field store: target place` (8)
- `structural field store: case field type` (6)
- `state graph: state signature: parameter signature: attached data shape` (5)
- `structural field store: destination parameter` (4)
- `state graph: terminator: guarded jump successors: roster` (4)
- `statement sequence: call: flow call` (4)

## One repair attempted, and where it is NOT

`DEFERRED-LOCAL-INITIALIZATION`, reduced from
`pass/wire/runtime_wire_utf8_edge_verdicts_exit`, was attempted and not
landed. The negative result is recorded because it is what saves the next
attempt.

The DECLARATION is fine. `let mut x: i32;` alone, and followed by unrelated
work, both plan; only the later `x = 1` has no route.

The RECORD half already works, through an explicit `declared_zeroed` branch
in `structural_scalar_store/destination.rs::owned_local_root` — a declaration
establishes the affine root, as its comment says — gated on
`Named | Generic`. The same function rejects any `primitive_type_reference`
outright, and should: a bare scalar local assignment is not a structural
field store. So widening that branch is the wrong repair, and it is exactly
where reading alone leads.

Instrumented rather than argued, in two rounds. The first marked all 44
`return None` exits across `structural_scalar_store/{mod,destination}.rs` and
diffed the failing program against the WORKING one
(`let mut x: i32 = 0; x = 1;`): same marks at nearly the same counts.

That alone did NOT support the conclusion, and the flaw is worth naming
because it is easy to repeat. Marking only declines cannot distinguish "the
working case never succeeds here" from "it succeeds after the same declines"
— the sentinel rule. A second round marked the SUCCESS constructions instead:
both programs reach the one success site exactly 1151 times, identically. Now
the conclusion holds. **The structural store subtree is not the repair
site**; it is merely the last route the trace entered, which is also why the
phase names a `destination parameter` for a local that is neither a field nor
a parameter.

A third round marked all 74 `return None` exits across the sequence planner
(`control/statement_sequence.rs`, `control/statement_sequence/local_data.rs`,
`terminal_unit/mod.rs`). No mark appears only in the failing run; every
difference is FEWER declines in it (920 against 922, 2202 against 2204),
which is a run doing less work, not a distinctive decline. So the explicit
exits there are not the site either.

None of those rounds covered the `?` operator, which is how most of this
codebase exits an Option. A fourth round found the site without marking a
single `?`: the existing `trace.phase` calls already sit at every meaningful
route point, so one edit to `control/construction_trace.rs::phase` prints the
whole phase SEQUENCE, and diffing the two programs' sequences shows exactly
where they part. Use that before reaching for `?` instrumentation.

**The site.** The working run reaches `statement sequence: local data:
mutable primitive local: storage initializer` and goes on to `statement
sequence: assignment`. The failing run never reaches that phase and drops
straight to `structural field store: target place`. That phase is set in
`control/statement_sequence/local_data.rs`, in the branch for a `mut`
primitive local, and the very next statement is the exit:

```rust
let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
    state.symbol, statement_index, CheckedScalarExpressionRole::StorageInitializer,
)?;
```

A declared-zeroed local has no initializer expression, so that lookup finds
nothing and the `?` bails. The local never emits `EstablishPrimitiveLocal`,
so every later assignment to it has no destination. That is the bug, and it
is why the failure surfaces at a *store* phase far from its cause.

**The fix shape.** The same function already computes `uninit_zeroed` at its
top — `is_mutable && !expression_is_valid(initial_value) &&
zeroed_structural_parts(..)` — and deliberately lets the case through, so
only this branch is missing it. When the initializer is absent, establish
with the carrier's landed zero rather than demanding a binding:
`IntegerLiteral::zero().with_landing(IntegerLanding { landed_type, domain })`,
`landed_type` from the existing `PrimitiveType -> LandedIntegerType` match in
`const_initializers/invocations.rs`, and `domain` from
`arithmetic_domain_for_type_reference` so an `in Wrapping` local keeps its
policy. The structural arm already does the equivalent for records, which is
why `let mut p: Plain;` works and `let mut x: i32;` does not.

NOT IMPLEMENTED HERE. `local_data.rs` is held by the active
LOCAL-DATA-CLUSTER-REPAIR claim, whose owner is repairing this very cluster,
and a claim on it was refused. This is a handoff with the site and the fix
shape, not a deferral.

## REFERENCE-LOCAL-BINDING-ROUTES, located by the same technique

The phase-sequence diff generalizes, so the biggest item's shared-borrow
trigger was located the same way, from its one-token pair:
`let d: &mut u32 = &mut self.size;` against `let d: &u32 = &self.size;`.

The working run leaves the local-data planner immediately after `statement
sequence: local data: initializer expression` and continues to `statement
sequence: borrowed windows closed`. The failing run instead walks `local
data: borrowed window move`, `view subslice`, `exclusive place loan`, and
dies at `local data: structural call binding`.

So the `&mut` form is admitted by the EARLY exit in
`control/statement_sequence/local_data.rs`:

```rust
if erased_locals.contains(&local.symbol) || is_record_pattern_marker(local) {
    return Some(LocalPlan::Planned);
}
```

An erased borrow carrier is a checked ALIAS rather than a storage binding —
its declaration composes through the sequence and each use joins the
referent's evidence. Membership comes from
`statement_sequence.rs::erased_alias_locals`, which is
`receiver_aliases::aliases` plus restored call aliases, and the gate is in
`terminal_unit/receiver_aliases/mod.rs`:

```rust
let access = exclusive_access(program, local.type_reference)?;
...
if !matches!((&access, borrow.access),
    (BorrowAccessKind::Mutable,   ReferenceAccess::Mutable)
  | (BorrowAccessKind::WriteOnly, ReferenceAccess::WriteOnly)) {
    return None;
}
```

A SHARED borrow matches neither arm, and `exclusive_access` bails on it
first, so it is never an alias, never takes the early exit, and falls through
to a route that has nothing for it. That is the whole of trigger 1.

CAUTION, and why this is a handoff rather than a patch. The alias machinery
is deliberately EXCLUSIVE — the module is named for it, the predicate is
`exclusive_access`, and a later site guards on `parent_borrow.access !=
Mutable`. Conceptually a shared borrow is an alias too, and the same
"declaration composes, uses join the referent" reading applies. But widening
this match is a change to the aliasing model, not a missing branch, and the
consumers sit inside the files LOCAL-DATA-CLUSTER-REPAIR currently holds. A
shared borrow may want its own route rather than being admitted through the
exclusive one. Decide that before editing the match.

## MEMBER-POLICY-REMOVES-COMPOSITE-ROWS, array half, located

One-token pair again: `self.row = [1, 2, 3]` into `[i32; 3]` against
`[i32 in Wrapping; 3]`.

The plain array is planned by the SCALAR field store sequence — its phases
run `scalar field store sequence: assignment store` and then straight on to
`statement sequence: assignment`. The policy element pushes the store out of
that route and into the structural one, which walks `structural field store:
target place`, `carrier path`, `field owner record`, `byte sequence
carrier`, `borrowed view field` and dies at `scalar field type`. The
structural route has no row for a whole-array store; the scalar route did,
and the element type is what decides which one is taken.

The gate is `primitive::array_literal_stores`, reached from
`structural_scalar_store/mod.rs`, and the predicate is
`is_closed_primitive_array_type` in
`checking/src/value_custody/literals/constant_arrays.rs`. It walks
`FixedArray` to its element and accepts only a `Named` node that is a bare
`Bool | I8..I64 | U8..U64 | F32 | F64`. An element spelled `i32 in Wrapping`
is a `Constrained` node, hits the `_ => return false` arm, and the array is
no longer a "closed primitive array". The same arm explains the RECORD
element case measured beside it: a record element is `Named` but not
primitive.

**HAZARD — do not widen that predicate.** It has 55 use sites across both
pipelines, and it means "array of BARE primitives" to every one of them.
Unwrap the element's shells at the `array_literal_stores` call site instead,
or give that route its own predicate; `value_custody/places.rs::
unwrapped_type_reference` is the existing helper for stripping the shell.
Changing the shared predicate would silently re-route 54 other decisions.

## Method notes for whoever re-runs this

- The phase string names the last phase ENTERED, not the operation that
  declined. Every attribution above is by fixture shape or by an existing
  reduction, never by the label alone.
- A machine with no terminal effect demands no unit plan, so a shrunk
  reproduction that drops the console binding checks VACUOUSLY. Keep one
  effect and the `roots.bind(...ProgramEntry...)`.
- Recording this roster costs two full native passes (one to see movement,
  one to record). Budget for that; the run is ~15 minutes of wall clock at
  nine compiles in flight, not the multi-hour figure the gate items imply.

## Full phase breakdown

### 26 — `statement sequence: local data: structural call binding`

Groups: recast 7, slices 7, wire 5, ownership 2, collections 1, dependent 1, float 1, text 1, traits 1

- `pass/collections/runtime_dutch_flag_partition_exit`
- `pass/dependent/zero_case_absorbs_nested_gate_compile`
- `pass/float/runtime_total_order_satisfiers_exit`
- `pass/ownership/linear_transfer_and_consume`
- `pass/ownership/linear_transparent_record_frontier`
- `pass/recast/runtime_aggregate_slice_representation_recast_exit`
- `pass/recast/runtime_bool_representation_recast_exit`
- `pass/recast/runtime_guarded_offset_recast_exit`
- `pass/recast/runtime_interior_slice_congruent_offset_exit`
- `pass/recast/runtime_interior_slice_view_mutable_write_exit`
- `pass/recast/runtime_mutable_equivalent_domain_recast_exit`
- `pass/recast/runtime_shared_domain_weakening_recast_exit`
- `pass/slices/runtime_nested_subslice_fixed_index_exit`
- `pass/slices/runtime_slice_index_copy_dispatch_exit`
- `pass/slices/runtime_slice_index_copy_exit`
- `pass/slices/runtime_slice_index_transition_exit`
- `pass/slices/runtime_slice_iteration_exit`
- `pass/slices/runtime_subslice_len_exit`
- `pass/slices/runtime_subslice_range_pointer_exit`
- `pass/text/runtime_owned_string_byte_view_exit`
- `pass/traits/runtime_local_named_dyn_devirtualized_exit`
- `pass/wire/runtime_wire_decode_ranged_field_exit`
- `pass/wire/runtime_wire_encode_borrowed_scalar_slice_exit`
- `pass/wire/runtime_wire_encode_byte_slice_exit`
- `pass/wire/runtime_wire_exact_array_without_count_exit`
- `pass/wire/runtime_wire_roundtrip_nested_and_repeated_exit`

### 10 — `state graph: terminator: conditional successors: guard expression`

Groups: calls 4, arithmetic 2, collections 1, domains 1, float 1, operators 1

- `pass/arithmetic/runtime_float_nan_comparison_exit`
- `pass/arithmetic/runtime_float_self_compare_nan_exit`
- `pass/calls/runtime_dispatch_float_terminal_exit`
- `pass/calls/runtime_std_math_sin_cos_exit`
- `pass/calls/runtime_transition_subject_call_single_evaluation_exit`
- `pass/calls/runtime_value_call_terminal_exit`
- `pass/collections/runtime_indexed_struct_field_rmw_exit`
- `pass/domains/utf8_equals_literal_exit`
- `pass/float/f32_per_operation_rounding_exit`
- `pass/operators/float_operator_identities`

### 8 — `scalar field store sequence: write frame agreement`

Groups: text 2, wire 2, collections 1, filesystem 1, slices 1, termination 1

- `pass/collections/runtime_frame_indexed_param_read_exit`
- `pass/filesystem/dir_walk_wrappers_exit`
- `pass/slices/domained_runtime_end_subslice_exit`
- `pass/termination/runtime_shrinking_slice_recursion_exit`
- `pass/text/runtime_local_struct_string_field_concat_exit`
- `pass/text/runtime_string_stored_suffix_exit`
- `pass/wire/runtime_wire_decode_byte_slice_exit`
- `pass/wire/runtime_wire_utf8_edge_verdicts_exit`

### 8 — `structural field store: target place`

Groups: layouts 4, recast 3, traits 1

- `pass/layouts/runtime_plan_laid_integer_at_proved_write_exit`
- `pass/layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit`
- `pass/layouts/runtime_plan_laid_record_array_mutable_write_exit`
- `pass/layouts/runtime_plan_laid_tiled_record_nested_array_view_exit`
- `pass/recast/runtime_mutable_equivalent_range_recast_exit`
- `pass/recast/runtime_offset_byte_recast_mutable_write_exit`
- `pass/recast/runtime_record_array_view_mutable_write_exit`
- `pass/traits/runtime_local_named_dyn_pass_through_exit`

### 7 — `state graph: terminator: conditional successors: parameter transfer`

Groups: filesystem 3, calls 1, data 1, text 1, time 1

- `pass/calls/runtime_nested_value_call_guard_exit`
- `pass/data/runtime_record_field_value_pattern_exit`
- `pass/filesystem/windows_canonicalize_exit`
- `pass/filesystem/windows_wrapper_results_exit`
- `pass/filesystem/wrapper_open_with_exit`
- `pass/text/runtime_param_domain_forward_exit`
- `pass/time/runtime_duration_core_exit`

### 6 — `structural field store: case field type`

Groups: calls 1, data 1, generics 1, ranges 1, relevance 1, traits 1

- `pass/calls/runtime_enum_self_method_exit`
- `pass/data/runtime_mixed_shape_exit`
- `pass/generics/runtime_mixed_generic_erased_sum_exit`
- `pass/ranges/sum_payload_range_arith_narrowed_exit`
- `pass/relevance/erased_case_payload_field_exit`
- `pass/traits/equatable_mixed_shape_equality_exit`

### 5 — `state graph: state signature: parameter signature: attached data shape`

Groups: recast 3, float 1, generics 1

- `pass/float/finite_core_domain_range_discharge`
- `pass/generics/runtime_const_container_methods_exit`
- `pass/recast/runtime_float_range_representation_recast_exit`
- `pass/recast/runtime_mutable_equivalent_record_recast_exit`
- `pass/recast/runtime_shared_record_float_range_weakening_exit`

### 4 — `state graph: terminator: guarded jump successors: roster`

Groups: control_flow 3, data 1

- `pass/control_flow/guarded_value_transition_roster`
- `pass/control_flow/runtime_sum_tuple_matrix_exhaustive_exit`
- `pass/control_flow/runtime_tuple_case_destructure_exit`
- `pass/data/match_exhaustive_by_case_union_domain`

### 4 — `statement sequence: call: flow call`

Groups: wire 3, dependent 1

- `pass/dependent/runtime_sibling_len_index_exit`
- `pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit`
- `pass/wire/runtime_wire_decode_rejects_wrong_era_exit`
- `pass/wire/runtime_wire_policy_authored_plan_exit`

### 4 — `structural field store: destination parameter`

Groups: borrow 1, calls 1, collections 1, slices 1

- `pass/borrow/runtime_method_view_write_after_last_use_exit`
- `pass/calls/runtime_reference_returned_slice_element_write_exit`
- `pass/collections/runtime_fixed_vec_round_trip_exit`
- `pass/slices/runtime_mutable_slice_element_write_exit`

### 3 — `call operation: boundary claim transfers`

Groups: generics 2, capabilities 1

- `pass/capabilities/win64_pointer_length_vs_descriptor_compile`
- `pass/generics/closed_indexed_quantity`
- `pass/generics/open_computed_quantity_result`

### 3 — `state graph: prefix initializers: bound expression`

Groups: text 2, types 1

- `pass/text/runtime_text_not_equals_exit`
- `pass/text/zii_default_string_equality_exit`
- `pass/types/runtime_addr_algebra_exit`

### 3 — `statement sequence: local data: scalar local: pure initializer`

Groups: calls 2, collections 1

- `pass/calls/transition_arg_local_from_embedded_call_exit`
- `pass/calls/value_call_embedded_in_binary_exit`
- `pass/collections/runtime_double_indexed_rmw_exit`

### 3 — `structural field store: byte sequence carrier`

Groups: text 3

- `pass/text/runtime_bounded_carrier_concat_exit`
- `pass/text/runtime_chained_string_append_exit`
- `pass/text/runtime_string_concat_two_fields_exit`

### 3 — `structural field store: pure source: scalar expression row`

Groups: expressions 1, float 1, types 1

- `pass/expressions/runtime_numeric_cast_exit`
- `pass/float/float_to_int_trapping_nan_traps`
- `pass/types/runtime_addr_field_exit`

### 3 — `structural field store: record literal source`

Groups: storage 1, structs 1, traits 1

- `pass/storage/runtime_machine_owned_indexed_struct_copy_exit`
- `pass/structs/runtime_struct_array_literal_exit`
- `pass/traits/equatable_string_field_equality_exit`

### 2 — `(no phase — reported through a callee chain)`

Groups: float 1, slices 1

- `pass/float/runtime_std_is_finite_exit`
- `pass/slices/runtime_subslice_dynamic_index_exit`

### 2 — `call operation: boundary arguments`

Groups: host 2

- `pass/host/runtime_gui_foreground_window_exit`
- `pass/host/runtime_gui_window_lifecycle_exit`

### 2 — `call operation: structural arguments: caller structural result`

Groups: calls 1, versioning 1

- `pass/calls/runtime_mutable_local_parameter_write_exit`
- `pass/versioning/runtime_version_migration_exit`

### 2 — `call operation: structural arguments: parameter path`

Groups: text 2

- `pass/text/runtime_bounded_carrier_alias_concat_exit`
- `pass/text/runtime_stdin_line_buffering_exit`

### 2 — `call operation: structural arguments: source symbol`

Groups: control_flow 2

- `pass/control_flow/arm_pattern_rest_optout_exit`
- `pass/control_flow/case_pattern_rename_waive_exit`

### 2 — `result ownership`

Groups: calls 1, text 1

- `pass/calls/case_scrutinee_param_clone_exit`
- `pass/text/runtime_stdin_command_branch_exit`

### 2 — `state graph: state signature: contract shape unadmitted`

Groups: range 1, slices 1

- `pass/range/runtime_guarded_runtime_index_increment_exit`
- `pass/slices/runtime_saturating_array_element_guard_exit`

### 2 — `state graph: terminator: jump successor: parameter transfer`

Groups: calls 1, dungeon 1

- `pass/calls/runtime_mut_ref_forward_exit`
- `pass/dungeon/runtime_threaded_mut_arg_interrupt_soak_exit`

### 2 — `statement sequence: local data: initializer expression`

Groups: calls 2

- `pass/calls/runtime_alias_write_through_guarded_transition_exit`
- `pass/calls/runtime_referenced_local_outlives_sibling_guard_call_exit`

### 2 — `statement sequence: local data: structural result shape`

Groups: storage 1, wire 1

- `pass/storage/runtime_local_slice_forward_exit`
- `pass/wire/wire_preserving_decode_relay_exit`

### 2 — `structural field store: borrowed view field`

Groups: control_flow 1, domains 1

- `pass/control_flow/runtime_string_literal_dispatch_exit`
- `pass/domains/utf8_field_read_carries_domain_exit`

### 2 — `structural field store: pure source: call result: result type`

Groups: filesystem 1, time 1

- `pass/filesystem/windows_set_file_time_exit`
- `pass/time/runtime_fs_mtime_system_time_interop_exit`

### 1 — `atomic access: place: shared receiver without atomic cell identity`

Groups: atomics 1

- `pass/atomics/shared_receiver_atomic_store`

### 1 — `call operation: structural arguments: alias and identity`

Groups: traits 1

- `pass/traits/runtime_dyn_two_impl_dispatch_exit`

### 1 — `completion`

Groups: capabilities 1

- `pass/capabilities/sysv_small_aggregate_import_compile`

### 1 — `outer calls: unconsumed nested call in a call statement`

Groups: calls 1

- `pass/calls/value_call_as_host_arg_exit`

### 1 — `outer calls: unconsumed nested call in a local initializer`

Groups: calls 1

- `pass/calls/nested_value_call_arg_compile`

### 1 — `signature`

Groups: generics 1

- `pass/generics/runtime_generic_multiple_specializations_exit`

### 1 — `state graph: natural ranks`

Groups: calls 1

- `pass/calls/runtime_value_call_statement_recursive_walk_exit`

### 1 — `state graph: state signature: parameter signature: parameter qualifications`

Groups: text 1

- `pass/text/runtime_mutable_string_parameter_concat_write_line`

### 1 — `state graph: terminator: conditional successors: scalar arguments`

Groups: calls 1

- `pass/calls/runtime_guarded_effectful_transition_argument_exit`

### 1 — `state graph: terminator: conditional successors: target state`

Groups: calls 1

- `pass/calls/runtime_dispatch_binary_call_argument_exit`

### 1 — `state graph: terminator: guarded jump successors: receiver transfer`

Groups: control_flow 1

- `pass/control_flow/runtime_tuple_transition_exit`

### 1 — `state graph: terminator: jump successor: scalar arguments`

Groups: float 1

- `pass/float/f32_arg_const_arith_landed_exit`

### 1 — `state graph: terminator: unsupported tail: guarded pair without exact false fallback`

Groups: control_flow 1

- `pass/control_flow/structural_result_transition_tail`

### 1 — `state graph: terminator: unsupported tail: jump followed by a transition`

Groups: control_flow 1

- `pass/control_flow/runtime_nonplace_record_pattern_single_evaluation_exit`

### 1 — `statement sequence: call count agreement`

Groups: slices 1

- `pass/slices/runtime_bounded_fixed_array_subslice_arg_exit`

### 1 — `statement sequence: local data: structural value: reference record loans`

Groups: expressions 1

- `pass/expressions/borrow_carrying_data_field_exit`

### 1 — `statement sequence: local data: structural value: value calls`

Groups: domains 1

- `pass/domains/runtime_result_domain_machine_overload_exit`

### 1 — `statement sequence: unsupported statement kind`

Groups: filesystem 1

- `pass/filesystem/wrapper_lock_metadata_exit`

### 1 — `structural field store: carrier path`

Groups: dependent 1

- `pass/dependent/data_where_invariant_window_restored_exit`

### 1 — `structural field store: scalar field type`

Groups: collections 1

- `pass/collections/runtime_whole_array_value_copy_exit`
