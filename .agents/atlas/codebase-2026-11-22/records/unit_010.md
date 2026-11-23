# Record: unit_010 (area 10-native-stages-04-10)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. All paths are under
`source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/`
(shortened to `04/src/`). The unit is eight files, 20,396 lines, all of them read in full.

Vocabulary used below: a "target row" is one `TargetUnitOperation` or `TargetControlTerminator` that stage 04
lowering wrote. The "audit" is the independent validator that re-derives each row from the abstract operation
and the optimization unit and compares. "Custody error" is `LegalizationError::custody()`, the one error the
audit raises for almost every mismatch (grep count of `LegalizationError::custody()` in `legalization_target.rs`: 186).

---

## 1. `04/src/preparation/legalization_target.rs` (7,559 lines)

**Purpose.** The target-plan half of the independent validator. For each function it takes the retained target
control graph and compares every row with the abstract operation and optimization-unit node it came from. It
is never called by default compilation (`preparation/mod.rs:13-18` says so; read). It also holds the data
structures the audit uses to know which scalar values are defined where. Header comment: lines 1-5.

**Shape.** One flat file of nested `pub(crate) mod`s (read):

| Range | Item | What it checks |
| --- | --- | --- |
| 18-234 | `byte_view`, `Checker::byte_view` (28), `mutable_byte_view` (180), `established_byte_view` (184) | `TargetByteView` variants Result/Owned/Literal/BlockParameter/Parameter/Subslice against the source |
| 235-1988 | `control_flow` | per-function block walk |
| 249-352 | `no_normal::validate` (258) | `InvokeNoNormal` terminator |
| 353-939 | `sources` | scalar-source bookkeeping, dominators |
| 940-1247 | `structural_cases` (`home_available` 955, `matches` 1074) | `StructuralCase` terminator |
| 1249 | `validate_instruction_metadata` | source annotations on each row |
| 1325 | `control_flow::validate` | the whole-function walk; returns `BlockEntryCustody` |
| 1948, 1975 | `successor_matches`, `edge_cleanup_matches` | Jump and Conditional edges |
| 1989-2621 | `element_view`: `element_view_access` (2025), `block_backing_precedes` (2332), `local_home_precedes` (2387), `established_element_window` (2434) | `TargetElementView` rows |
| 2622-3193 | `expressions`: `Checker::expression` (2766), `boolean` (2916), `float_element_read` (2732), `operands_agree` (3137), `arithmetic_carrier_admitted` (3177) | scalar expression trees |
| 3194-3311 | `hosted_scalar::validate` (3206) | hosted write-byte / exit-process rows |
| 3312-3573 | `normalized_foreign::validate` (3327) | `NormalizedForeignCall` rows |
| 3574-3969 | `scalar_definitions`: `validate` (3589, IntegerWiden only), `observed_family` (3708), `observation` (3726) | `ScalarDefinition` rows |
| 3970-7369 | `unit` | per-operation audit |
| 3991-4648 | `unit::aggregate_results::validate` (4007) | activation-claim, native-code-layout, array, record, scalar-array, leaf-copy, case establishment |
| 4649-4755 | `borrowed_windows::validate` (4657) | `MoveStructuralField`, `StoreStructuralField` |
| 4756-5191 | `direct_calls::validate` (5016), `results::validate` (4776) | authored `Call` rows |
| 5192-5359 | `ieee_float::validate` (5200) | float policy, compare, representation view, constant |
| 5360-5580 | `primitive_store::validate` (5369) | `WriteOnlyPrimitiveStore` |
| 5581-7299 | `validate_operation` | the dispatch over (target row, abstract operation) pairs |
| 7301, 7333, 7352 | `validate_reference_indices`, `runtime_indices_rejoin`, `scalar_home` | helpers |
| 7370 | `validate_target` | entry for one function: provenance check then `control_flow::validate` |
| 7426 | `struct Checker<'a> { function, available: Option<&AvailableSources>, optimized, function_inputs, types }` | shared checker context |
| 7435, 7442 | `resolve`, `location_matches` | alias and parameter-location helpers |
| 7450-7559 | `decomposition_tests` | one test |

Key types: `AvailableSources` (472; inherited rows plus a local `Vec<SourceRow>`), `SourceInputs` (439; all
inherited rows once, scoped per block), `SourceDominators` (696; `order: BTreeMap<BlockId, usize>` plus subtree
intervals), `Checker` (7426).

**Inputs and outputs.** Reads `TargetFunction`/`TargetControlGraph`, `AbstractFunction`,
`PsiOptimizationFunction` (the "optimized" function), `FunctionInputs`, `DescriptorInputs`, and the
`NativePlan`/`NativeUnit` wrappers from `preparation/mod.rs:81-132`. Output is `Result<BlockEntryCustody,
LegalizationError>` (7375). It mutates only the local `AvailableSources` and the `Custody` state as it walks.

**How it decides.** It decides by equality of identities and of whole rows with a re-derived expectation.
- `validate_target` (7370-7424) compares the sorted list of operation ids and edge ids in `target.provenance`
  with those of the optimized function (7411-7415).
- `control_flow::validate` (1325) requires matching block count, entry, attachment and structural types
  (1347-1354), then for each block pairs target block with source block by position (1416, `zip`) and checks
  `block.block != source.id` (1449).
- Rows are keyed on `psi_operation`, `psi_edge`, `PlaceId`, `ValueId`, `BlockId`, `BoundaryMachineId`
  identities, never on display names. Example: `byte_view` Result arm `*place == source &&
  descriptor_result(...) == Some((*psi_operation, *structural_type))` (35-43).
- A scalar value is accepted only if `available.matching(value)` yields a row equal to the row the target
  named (for example 313-315, 5111-5113, 6024-6028).
- Dominance is by block id through `SourceDominators::dominates` (818) and `sources::dominates` (688).
- One judgment keys on a spelling-free "family": `scalar_definitions::observed_family` (3708) is a `matches!`
  over abstract operation variants.

**Cases by shape.**
- `hosted_scalar::validate` (3243-3274): "exactly one i32 argument" (`let ([value], [argument]) = ...`, 3243), only
  `HostedWriteByteI32` and `HostedExitProcessI32`, result `Unit`, declaration must have exactly `[i32]` scalar
  parameters and nothing else (3283-3301).
- `scalar_definitions::validate` (3589-3706): only `IntegerWiden` into 16, 32 or 64 bits (`matches!(target_integer.bits(), 16 | 32 | 64)`, 3632);
  operand must be a Parameter, Immediate, BlockParameter or ScalarHome (3647-3701).
- `established_byte_view` (184-232): the element must be unsigned 8-bit (`IntegerType::new(Unsigned, 8)`, 224-228).
- `Expression::ByteSequenceLength` (2788-2804): comment "A bounded view's length is its exact capacity" and
  returns false for `BoundedView` (2796).
- `ReturnScalar` terminator: `IeeeFloatElementViewRead { .. } => false` (1804), so a float element-read cannot be returned directly.
- `validate_operation` (5581-7299) is a closed list of (target, abstract) pairs. The last arm is `_ => return
  Err(LegalizationError::custody())` (7296). Several boundary-call arms are fixed shapes: `ProgramImageAddress`
  requires `native.profile == Some(UefiX64)`, no arguments, result `address(64)` (5829-5847);
  `StaticInterrupt` requires exactly one token structural argument (5712) and refuses `Install` (5710).
- `aggregate_results::validate` EstablishStructuralArray accepts only `FixedArray` with literal length or `ElementView{access: None}` (4176-4197), else Err (4196).
- `aggregate_results::validate` EstablishCase accepts only `Sum` or `Mixed` (4459-4465).
- `direct_calls::validate` only `NativeCallOrigin::Authored` rows (5028).
- `ieee_float::validate`: `ValueShape::float` width only for Binary32 (4) and Binary64 (8) (5221-5224).

**Same job elsewhere.**
- Dominator computation also appears at `04/src/target_lowering/control_flow.rs:75-352` (`dominance::schedule`,
  bitset sets, iterative until fixed). `SourceDominators::new` (729-810) uses reverse postorder with the
  intersect method over `BTreeMap` and a parent array, then `tree_intervals` (704). Two implementations in one
  crate (read both). A grep for `dominance::schedule|SourceDominators|fn intersect|immediate dominator|reverse_postorder|fn dominates` finds 26 files
  across `source-rust/` including `psi-to-native/representations/abstract-operations/optimization-unit/src/dominance.rs`
  and `compiler/psi/semantics/src/control_graph/dominance.rs`; I did not read those two (suspected same job).
- The audit repeats the lowering's checks. Pairs (both read): `ProgramImageAddress` lowering
  (`control_flow.rs:473-545`) and audit (`legalization_target.rs:5803-5851`); `StaticInterrupt` lowering
  (`control_flow.rs:720-827`) and audit (5678-5750); `ExternalCompletion` terminator lowering
  (`control_flow.rs:1577-1704`) and audit (`legalization_target.rs:1605-1742`); structural-case lowering
  (`control_flow.rs:1045-1229`) and audit (`legalization_target.rs:1074-1228`).
- `external_result_is_erased` is called directly at `legalization_target.rs:1715` and via
  `validate_external_completion_result` in lowering (`control_flow.rs:1514`).
- The Boolean/Integer immediate "arithmetic carrier" check is written twice: `arithmetic_carrier_admitted`
  (3177-3192) and an inline copy at 3747-3757 (both read, same two match arms).
- The `StructuralScalarFieldStore` and `WriteOnlyPrimitiveStore` storage-source `match` (6123-6194 and 5401-5464)
  repeat the same four-way (Parameter+Placement, Parameter+BlockParameter, OwnedResult+StructuralHome,
  OwnedBlockParameter+BlockParameter) structure with different access conditions.

**Unused or one-caller code.**
- `validate_target` (7370): grep `validate_target` across `source-rust/`: not run separately; it is `pub(crate)`
  and reached from `preparation/legalization.rs` (inferred, that file is outside the unit). Count not taken.
- `runtime_indices_rejoin`: 19 matches in this file and none elsewhere (Grep count `fn runtime_indices_rejoin|runtime_indices_rejoin\(` over `source-rust/`: 19 in `legalization_target.rs`, 0 in other files).
- `OMEGA_COMPILER_CHECK_SITES` debug printing at 4974 (Grep count over `source-rust/`: 30 occurrences in 10 files; 1 here).
- `sources::call_indices` (358), `sources::dominates` (688): `dominates` is used at 996, 1022, 1551, 2384, 2430 in this file (read).

**Stated limits.**
- "Decomposition has no bytes to witness its subject later. Bind the retained row here" (1271-1272).
- "A bounded view's length is its exact capacity." (2795).
- "State arrivals transport the same owned values as field inputs." (2540; in `control_flow.rs` not this file; see section 5).
- "Selection reads the result, selector operands, extents and proof keys from this row alone, so each must rejoin here." (4412-4413).
- "Selection derives the store's shape from the retained value type, so it must name the written field's type." (4742-4743).
- "view array extent is still a rigid const input" (2559, 2508-2509) refuses arrays whose length is a parameter.
- "representation view source has an unsupported live extent" (2517).

**Fail-open risks.**
1. `SourceDominators::dominates` returns `true` if `block` is not in `order` (822-824: `let Some(position) = ... else { return true; }`).
   Input that reaches it: a block not reachable from the entry. Classification: open for audit input (no comment at the site). Lowering refuses
   unreachable blocks at `target_lowering/control_flow.rs:84-86` and `3061-3062` (read), so an unreachable block does not appear in lowering's output; the audit is
   also the check for hand-built plans in tests, so I did not trace that case.
2. `Boolean::Immediate` check (2977-2979): `self.available.is_none_or(...)` accepts when `available` is `None`. Traced: all three `Checker { ... }`
   constructions in the crate set `available: Some(...)` (1796, 1928, 5627; Grep `available: None|Checker \{` over `04/src`: 3 hits, none `None`).
   Classification: traced, not reachable today.
3. `arithmetic_carrier_admitted` (3190, `_ => true`) and the copy at 3756 admit every policy/operator pair except the three listed (saturating
   multiply/divide/remainder; wrapping divide/remainder). A new operator would be admitted by default. Classification: open.
4. `validate_operation` arms for `EstablishByteSequenceConcatenation` (6592-6611) and `GlobalDescriptorTable` (6809-6826), `PortWrite` (6182-6198), `ReleaseActivationClaim` (4050-4065) only compare fields for equality with the abstract
   operation. They check no further property of the field values. Classification: open; the equality is the whole contract.
5. `byte_view` Parameter arm (111-130) and `element_view` Parameter arm (2080-2104): accepted when the parameter list from `structural_parameters(self.function)` has a matching place and placement. If that function returns `None` the arm is false (closed).
6. Other `Option`/missing-row cases I read return `false` or a custody error (for example 822-826 is the only `true` on a missing lookup). None other seen.

---

## 2. `04/src/preparation/mod.rs` (226 lines)

**Purpose.** Module root of the preparation half. Declares the modules and holds the `native_geometry` wrapper
that carries "the module's retained integer types" (`ModuleTypes`) into the audit.

**Shape.** `native_geometry` (26-196): `ModuleTypes(ClosedTypeCatalog)` (38) with `integer` (41), `scalar` (48),
`retains` (56), test-only `of` (62); `accepted_types` (71); `NativeInput<'a, T>` (81) with `with_types` (87),
`closed_scalar` (98), `closed_integer` (105), `geometry` (116); aliases `NativePlan`, `NativeUnit` (131-132); one test (139).
Re-exports at 197-223 (`PreparedCallableInvocations`, `capture_source_publication`, `BoundaryOriginFacts`,
`LegalizationError`, `PreparationEvidence`, `prepare_execution_plan`, `verify_execution_plan`,
`verify_target_operation_plan_with_closed_module`, `accepted_types`). `sum_layout_selection` is `#[cfg(test)]` (225-226).

**Inputs/outputs.** `accepted_types(closed, psi, module)` returns `Some(ModuleTypes)` only if
`closed.identity() == psi && std::ptr::eq(closed.module(), module)` (76-77).

**How it decides.** By identity of the Psi program and by pointer identity of the module owner (76-77); the
comment says a module with equal bytes but another owner is refused (67-70) and the test confirms (190-193).

**Cases by shape.** None.

**Same job elsewhere.** Grep `accepted_types|ModuleTypes` over `source-rust/`: 36 occurrences in 8 files, all in this crate (`legalization.rs` 10, `mod.rs` 11, `sum_layout_selection.rs` 5, `legalization_source.rs` 4, `legalization_instruction.rs` 3, `legalization_control.rs` 1, `legalization_scalar_graph.rs` 1, `target_lowering/mod.rs` 1).

**Unused or one-caller code.** `ModuleTypes::retains` (56): not grepped separately.

**Stated limits.** None. Header says the validator "never" runs in ordinary compilation (13-17).

**Fail-open risks.** None seen. `ModuleTypes::integer`/`scalar` turn a closed-catalog error into `None` (45, 52); callers in this unit treat `None` as a custody error (for example `scalar_definitions.rs` 3623-3627).

---

## 3. `04/src/preparation/publication.rs` (405 lines)

**Purpose.** Records, before the working body is dropped, where each source operation and edge sits (ordinal in
its function), the quoted bytes of byte-sequence literals and the call target of each structural call. The
audit later recomputes these from the abstract plan and compares.

**Shape.** `capture_source_publication(plan) -> Result<NativePublicationFacts, String>` (9); `verify_source_publication(plan, facts) -> Result<(), &'static str>` (91); four tests (242, 261, 308, 340).

**Inputs/outputs.** Reads `AbstractOperationPlan.functions`, `dynamic_dispatch`, `callables`. Output sealed `NativePublicationFacts` (`target_vocabulary`) with per-function `sites`, `byte_literals`, `call_targets`.

**How it decides.** By identity and position: sites are `(PsiProvenance::{Operation, Edge}, ordinal)`; sorted and deduped (69-72). Ordinal is the position of the operation in `function.operations()`.
The comment at 70-71 states: a conditional or case dispatch may repeat one edge within the same source operation; "Reusing it at another ordinal remains an error."

**Cases by shape.** `capture` (29-46): an operation with no `psi_operation()` takes its edges from `operation.cleanup_rosters()`; if that gives none, it must be one of `InvokeNoNormal | ExternalCompletion | EliminateEmpty | Crash`, else the error "publication operation has no semantic occurrence" (42).
`verify` (135-154) lists Return patterns, `ReturnStructural`, `Jump`, `InvokeNoNormal`, `ExternalCompletion`, `EliminateEmpty`, `Crash`, `Conditional`, `StructuralCase`, else error "source operation has no publication occurrence" (153). The two lists differ in form: capture goes through `cleanup_rosters`, verify lists variants.

**Same job elsewhere.** `capture_source_publication` is called at `target_lowering/coordination.rs:621` (production) and `sum_layout_selection.rs:4225` (test) (Grep over `source-rust/`: 7 matches, plus the definition, the re-export at `preparation/mod.rs:202` and 4 in-file tests). `verify_source_publication` is called at `preparation/legalization.rs:771` and `:868` (Grep: 2 non-test callers).

**Unused or one-caller code.** `capture_source_publication`: 1 production caller. `verify_source_publication`: 2 callers, both in `legalization.rs`.

**Stated limits.** None other than the comment at 70-71.

**Fail-open risks.** None seen. Missing rows cause `Err` (125, 130, 158, 172-173, 183-187).

---

## 4. `04/src/preparation/sum_layout_selection.rs` (4,764 lines)

**Purpose.** Tests only (`#[cfg(test)] mod sum_layout_selection;` at `preparation/mod.rs:225-226`). Builds target rows by hand, runs the audit and, for the sum tests, runs preparation and instruction selection and compares the printed selection with `sum_layout_selection.expected.txt` (`include_str!`, 1092; the file exists, found by Glob in the same folder). Header (1-6): "The expected text is the selection `origin/main` emitted for that same geometry."

**Shape.** 29 `#[test]` functions between lines 41 and 4587 (counted by reading) and helpers. Tests, with starting lines:
`a_literal_edge_can_retire_its_descriptor_home` 41, `byte_literal_lookup_refuses_every_producer_the_scan_refused` 130,
`retained_instruction_metadata_projects_without_a_node_and_rejects_changed_custody` 254, `retained_interrupt_geometry_does_not_authorize_an_unassociated_helper` 455,
`retained_terminator_projects_without_a_body_and_audit_refuses_changed_evidence` 572, `function_storage_contract_projects_without_source_and_audits_every_field` 646,
`selection_honors_the_stored_call_plan_without_recomputing_it` 733, `scalar_parameter_projection_uses_retained_abi_without_a_source_function` 820,
`retained_call_descriptor_origins_do_not_need_old_bodies_but_audit_does` 868, `stored_sum_rows_select_the_same_instructions_as_main` 1080,
`retained_scalar_store_geometry_is_checked_against_its_source` 1096, `retained_field_store_value_type_is_checked_against_its_source` 1234,
`retained_leaf_copy_selectors_are_checked_against_their_source` 1358, `retained_byte_view_store_projects_and_audits_exact_operands` 1492,
`retained_storage_copies_stores_and_tag_tests_project_without_bodies` 1673, `retained_sequence_lengths_project_with_only_their_rows_and_facts` 1811,
`retained_element_view_reads_use_the_exact_descriptor_index_and_leaf` 1933, `retained_primitive_store_projects_indexed_leaf_and_audits_exact_geometry` 2155,
`retained_direct_byte_access_projects_and_audits_exact_operands` 2347, `retained_byte_field_read_preserves_both_bounds_and_audits_the_field` 2570,
`retained_byte_length_audit_reconstructs_the_descriptor_and_exact_fold` 2796, `retained_view_inputs_project_without_bodies_and_keep_independent_custody` 2920,
`retained_reference_selectors_project_without_bodies_and_audit_exact_geometry` 3218,
`ordinary_comparison_metadata_requires_the_module_to_retain_its_operand_type` 4515, `qualified_numeric_replay_needs_the_module_and_actual_numeric_meaning` 4587.
Fixture builders: `tag_only` 3436, `payload_behind_pointer` 3442, `tag_only_behind_pointer` 3461, `sum_inside_record` 3474, `membership_behind_pointer` 3516, `establish_case` 3566, `activation_claim` 3609, `dispatch` 3676, `select` 3850 (forges region size and pointer slots then asserts the validator refuses, 3857-3904), `validate_stored_geometry` 3788, `target_plan` 4127, `fixture_types` 4718, `module_retaining` 4735.
(The name `ordinary_comparison_metadata...` is a 24th-plus test; the exact count of `#[test]` is not grepped. Count by Grep not taken, so treat "29" as unverified: see Unsure.)

**Inputs/outputs.** Reads production functions (`prepare_execution_plan`, `legalize_target_operations`, `audit_target_operation_plan`, `validate_call_descriptor_origins`, `project_*` in `legalization_source`, `validate_operation`) and `instruction_selection::test_support::select_instructions_for_plan` (3934). Output is assert/panic.

**How it decides.** Each mutation test builds an exact row, asserts the audit passes, then mutates one field at a time (indices stride, extent, obligation, offset, source place, access) and asserts failure.

**Cases by shape.** Fixtures are fixed small shapes: `HOST = NativeTarget::linux_x64` (39); sum with two cases `Left`/`Right` (4399-4413); `u32`/`u64` fields (4460-4466).

**Same job elsewhere.** `validate_stored_geometry` (3788) re-runs `dispatch_projection` and `membership_layout` (the same calls as the audit's `structural_cases::matches` at `legalization_target.rs:1136` and `unit` at 6303).

**Unused or one-caller code.** Test file; nothing `pub`.

**Stated limits.** "full call ABI construction is covered by the kept returned-window program, not this deliberately minimal target row" (866-867). "These hand-built selection rows omit unrelated provenance metadata." (3796).

**Fail-open risks.** None seen (tests).

---

## 5. `04/src/target_lowering/call_result_origins.rs` (184 lines)

**Purpose.** After lowering, fills in `descriptor_origins` on each structural call whose result is a nonlinear
view-carrying type, by running the descriptor analysis over the optimized catalog graph. The module header
(1-4) says the graph borrows the stage's existing optimized input and only backing alternatives travel forward.

**Shape.** `pub(crate) fn retain(source, unit, selected, candidates, target) -> Result<(), LoweringError>` (17);
`fn project(machine, result, origin) -> Result<TargetDescriptorOrigin, LoweringError>` (154).

**Inputs/outputs.** Reads the abstract plan, optimization unit, `SelectedCallsByCall`, candidate callee functions.
Mutates `target.functions[*].graph.blocks[*].operations[*].kind` `Call { result: Structural { descriptor_origins } }`.

**How it decides.**
- Finds the first call whose result multiplicity matches `nonlinear_multiplicity_pair_pattern!()` and whose descriptor `Views` slots are non-empty or unresolved (`is_none_or(|slots| !slots.is_empty())`, 38-47). If none, returns `Ok(())` early (51-53).
- Origins come from `DescriptorAnalysis::try_call_origins(machine, place)` (143-146); keyed on `(MachineId, PlaceId)`.
- `project` (154-183): Empty, Literal, Argument map to target origins; `Invocation { .. } | AccessFormation { .. }` give the error "call result backing has no supported storage origin" (176-182).

**Cases by shape.** `project` supports three of five `DescriptorCallOrigin` shapes (159-175); `Argument` origin is forced to `StructuralAccess::SharedBorrow` and `without_runtime_coordinates()` (165-170).

**Same job elsewhere.** `validate_call_descriptor_origins` (in `preparation/legalization.rs`, not read) re-derives the same origins (used at `sum_layout_selection.rs:1005`). Same selected-call resolution as `control_flow_operations.rs:321-329` via `selected_boundary_calls::resolve` (line 79 here).

**Unused or one-caller code.** `retain`: Grep `call_result_origins` over `source-rust/`: 1 call (`target_lowering/coordination.rs:692`) and the `mod` line (`target_lowering/mod.rs:41`).

**Stated limits.** "call result backing has no supported storage origin" (180); "Only selected boundary callees need this lookup." (59).

**Fail-open risks.** `.ok().map(...)` at 87 turns a `resolve` error into `None` for that node. Classification: traced to the following `ok_or_else(... "descriptor backing graph is unresolved")` at 93-99, which fails the whole function if `optimized_catalog_graph` returns `None`; I did not read `optimized_catalog_graph` to confirm it returns `None` on a `None` node (inferred).

---

## 6. `04/src/target_lowering/control_flow.rs` (3,463 lines)

**Purpose.** Builds the target control-flow graph for one function: for each block, lowers each operation to
target rows, lowers the terminator, and iterates to a fixed point over how reference-descriptor storage flows across edges. Header (1-11).

**Shape.** Layout (read):
- 33-49: `#[path]` modules `control_flow_operations.rs`, `control_flow_calls.rs`, `control_flow_storage.rs` (not in this unit), `references.rs`.
- 51: `binding_indices`.
- 75-352: `dominance::schedule` (78), `reverse_postorder` (118), `solve_sets` (135, bitset), `dominator_positions` (183); tests (197-351) compare with `original_schedule` over every graph up to 4 nodes (301-314).
- 354-459: `interrupt_installation::lower` (367), `scope` (445).
- 461-545: `program_image::lower` (473).
- 548-632: `row::{RowParts, RowView, RowSite}` (570), with `establish_home` (615).
- 634-707: `run_custody::lower` (643).
- 709-828: `static_interrupts::lower` (720).
- 830-1230: `structural_case::{observe 848, parameter_root 900, case_source 909, case_projection 972, prepared_pointer_slots 1026, lower 1045}`.
- 1232-2159: `terminator::{disposable_view_carrier 1252, retirable_reference_arrival 1297, owned_root_type 1330, plain_home_cleanup 1369, validate_external_completion_result 1499, lower_terminator 1528}` and a test (2074).
- 2161-2691: `transfers::{validate_parameters 2172, re_home_bound_references 2197, enter 2293, is_address_join 2320, address_parameter 2329, address_join_argument 2340, validate_successors 2421}`.
- 2698: `ReferenceEdgeStorage`; 2708 `ScalarHomes = Arc<BTreeMap<...>>`; 2711 `LiveDefinitions` (17 fields; `BTreeMap`/`BTreeSet` keyed by `PlaceId`/`ValueId`); 2733 `impl LiveDefinitions`; 2826 `PendingControlBlock`.
- 2834-3463: `pub(crate) fn lower(function, optimized, target, profile, functions, dynamic_dispatch, callables, ..., installed_interrupts) -> Result<TargetFunction, LoweringError>`.

**Inputs/outputs.** Reads `AbstractFunction`, `PsiOptimizationFunction`, `StructuralTypeLookup`, boundary declarations, `BoundarySettlementBinding` map, `SelectedCallsByCall`, scalar ABIs, native-callback map, `AdmittedInstalledInterruptCohort`. Produces `TargetFunction { graph: TargetControlGraph, provenance, .. }` (3447-3462). `structural_contract`, `scalar_abi` and `mixed_structural_scalar_abi` are set to `None` here (3450-3452) and filled by the caller.

**How it decides.**
- Block and place identity: every scalar `ValueId` must be defined once (`definitions.insert`, 2876-2920, error `DuplicateValue`) and every structural `PlaceId` once (2921-2986).
- Block order: blocks must be strictly increasing by id (`entries[position - 1].block >= entry.block`, 3001).
- Dominance by position index through `dominance::schedule` (3061), which returns `None` if the reverse-postorder reach is smaller than the block count (84-86), i.e. an unreachable block is refused.
- Reference-storage joins: `merge_arrivals` / `merge_borrowed_arrivals` at predecessor joins; the loop (3126-3344) repeats until `reference_entries`, `borrowed_entries`, `drained_entries` stop changing, and errors if any block was `deferred` when nothing changed (3338-3341).
- Cleanup actions are judged on the owned-root type in `live` (`owned_root_type`, 1330) and on `structural_types.subtree(root, path) == Some(discard.structural_type)` (1365).
- Descriptor-return source is judged on matching `structural_type` and multiplicity (1810) with the shape `ValueShape::integer(16, 8)` (1837).

**Cases by shape.** Quoted or matched lists:
- Function acceptance: `Unit | Scalar | Structural` result and every structural parameter either a non-owned nonlinear borrow or `is_owned_parameter` (2858-2872).
- Block structural parameters (non-entry only) must be one of: byte parameter, immutable byte carrier, element-view parameter, descriptor join, address join, owned byte parameter, shared view carrier loan, owned parameter (2934-2951).
- Terminators accepted (3011-3043): `ReturnStructural | InvokeNoNormal | ExternalCompletion | EliminateEmpty | Crash`, `Return`/`ReturnUnit` only when every cleanup action is one of four kinds (3017-3026), `Jump`, `Conditional`, `StructuralCase`; else `unsupported_control_flow`.
- `lower_terminator` (1576-2061): `_ => Err(unsupported_control_flow)` (2061). Conditional's condition must be a Boolean parameter, constant, home or block parameter (2007-2042).
- `structural_case::lower`: refuses `scalar_descriptor` pointer slots (1066-1072); requires equal case counts (1125).
- `program_image::lower`: profile must be `UefiX64` (507).
- `static_interrupts::lower` requires one structural parameter and one token argument (763-768).
- `unsupported_control_flow` appears 79 times in this file (Grep count over the file; 152 across this unit's three `control_flow*.rs` files: 79+49+24).

**Same job elsewhere.** See section 1: `dominance::schedule` vs `SourceDominators` (legalization_target.rs:696-831); audit mirrors for `program_image`, `static_interrupts`, `ExternalCompletion`, `StructuralCase`. `validate_external_completion_result` (1499) has callers at `control_flow.rs:1590`, `target_lowering/coordination.rs:119` and four test calls (2111-2146) (Grep over `source-rust/`).

**Unused or one-caller code.** `validate_external_completion_result`: 2 production callers (1590 and `coordination.rs:119`).

**Stated limits.**
- "Block numbers can run opposite to control flow. Use graph order for convergence" (116-117).
- "Arrival inference may rebuild a block. Attach owned source annotations only after it settles" (2823-2824).
- "The chase addresses `regions.root`. That is the sum itself only when the path never leaves the sum's own storage" (1105-1109): selection "no longer walks the type" to recover region size.
- "Copying a descriptor value does not acquire its backing." (2224).
- "State arrivals transport the same owned values as field inputs." (2540).
- "The declaration retains Never (or another proved-empty type). Only its physical result lane is absent." (1512-1513).
- "Keep one row per source operation so the checked partition ... survive without bytes." (826-827, in `control_flow_operations.rs`).

**Fail-open risks.**
- Lines 3153-3158 and 3160-3175: a failing `reference_arrival` or `merge_arrivals` sets `deferred = true` and skips the block. Classification: traced; the loop errors with `unsupported_control_flow` if it ends with `deferred` (3338-3341), and the post-loop check recomputes arrivals and compares to the entries (3345-3370).
- `plain_home_cleanup` `RetireHome` arm (1377-1409): the clean path removes `live.references` entries for the retired place without further checks when `owned_root_type`, `retired_reference_roots`, `disposable_view_carrier`, or `retirable_reference_arrival` holds. Classification: traced, guarded by that disjunction.
- `reference_arrival` (2764-2797): when `reference_edge_exits` is empty it returns the current state scoped to `storage` (2775-2777) without per-edge check. Classification: open (no comment); reached for blocks whose predecessor recorded no per-edge exits (for example `Jump` terminators record none; `Jump` calls `re_home_bound_references` instead, 1981).
- Debug `eprintln!` guarded by env var `OMEGA_COMPILER_CHECK_SITES` at 1384 and 1415 (diagnostic only; not a fail-open).

---

## 7. `04/src/target_lowering/control_flow_calls.rs` (2,233 lines)

**Purpose.** Lowers calls inside the control graph: direct calls, calls through callable values and dynamic dispatch, no-normal invocations, cleanup calls, plus how each structural argument is backed (storage, view, owned, borrowed) and how a selected boundary call becomes an ordinary call. Header (1-9).

**Shape.** Modules (read):
- `borrowed_calls` 12-606: `direct_referent` 27, `argument` 56, `storage_argument` 226, `address_join_window_argument` 308, `stored_descriptor_argument` 339, `byte_argument` 425.
- `byte_arguments::shared_owner_view` 608-770.
- `call_signature` 772-909: `PreparedCallSignature`, `prepare` (800).
- `calls` 911-1935: `lower_no_normal` 952, `Invocation` 1051, `lower_call` 1064, `land_result` 1308, `published_scalar_contract` 1395, `require_exact_target_abi` 1486, `ordinary_contract` 1497, `structural_result_home` 1551, `result_qualifications_match` 1577, `scalar_argument` 1607, `structural_argument` 1631, `lower_cleanup_calls` 1854.
- `owned_arguments` 1937-2080: `is_owned_parameter` 1945, `argument` 1953.
- `selected_boundary_calls` 2082-2233: `resolve` 2090, `retain_origin` 2197.

**Inputs/outputs.** Reads `LiveDefinitions`, `PreparedFunctionSignature`, `StructuralTypeLookup`, callee `AbstractFunction`, `ScalarFunctionAbi` map. Pushes `TargetUnitOperation::Call` / `CleanupCall` / `TargetControlTerminator::InvokeNoNormal`.

**How it decides.**
- `structural_argument` (1631-1851) is an ordered if-ladder keyed on what the argument place is (view carrier parameter, home, address join, block view, entry parameter) and the formal's type (`is_view_descriptor`, reference, owned, primitive reference, byte parameter, element-view parameter). First match wins (1664-1850). Keyed on `PlaceId` membership in `live.structural_homes`, `live.views`, `live.block_views`, `live.address_joins`, `prepared.parameters`, and on the declared type shape.
- `lower_call` (1064): call target is one of `Machine`, `Callable { contract }`, `Dynamic { interface, slot, selector }` (1129-1165). Dynamic selector must be `Unsigned` 64-bit (1158-1161). The slot is looked up and `requirement.slot == slot` (1155).
- `published_scalar_contract` (1395): a published scalar call must have no structural parameters or entry claims on the callee (1406-1409), result placement must be one register location of the exact size (1450-1458), and the callee's retained scalar ABI must equal the re-derived one (`require_exact_target_abi`, 1491).
- `lower_cleanup_calls` (1854-1934): only `InvokeOwned`; callee must have exactly one structural parameter at position 0 with type equal to `cleanup.structural_type`, result Unit, no scalar parameters, no claim transfers, and the type's `nominal_cleanup` must name that machine and declaration (1879-1894).
- `selected_boundary_calls::resolve` (2090-2195): rewrites a `BoundaryCall` into `Call`, `CallStructural` or `CallStructural` by result shape match; keyed on `structural_type`+`multiplicity` equality (2147-2148) or `scalar_type` equality (2167). Chooses `AbstractOperation::Call` only if there are no structural arguments and no claim transfers (2168).

**Cases by shape.**
- `resolve` handles three result pairings: Unit/Unit, Structural/Structural, Scalar/Scalar; `_ => Err(invalid())` (2191).
- `byte_argument` carrier size is 8 for `BoundedView`, else 16 (446-451); destination shape must equal `borrowed_reference(carrier_size, 8)` (455).
- `shared_owner_view` (627-769): only `SharedBorrow` of a bounded owned byte sequence with no `Referent` or `FixedRange` in the path, root region with no pointer slots (704-709), `destination.shape == borrowed_reference(16, 8)` (741).
- `call_signature::prepare` result: structural result or scalar result only (836-848).
- `published_scalar_contract`: "result_placement" must be exactly one register location (1450).

**Same job elsewhere.**
- The access-kind `match` (`MutableBorrow => argument.access != Owned`, `SharedBorrow => ... == SharedBorrow`, `WriteOnlyBorrow => ... == WriteOnlyBorrow`, `Owned => ...`) is written out at 209-216, 257-264, 554-561 here and again in `legalization_target.rs` for stores (`5402-5428`, `7123-7158`, different conditions) (read).
- `selected_boundary_calls::resolve` is also called from `call_result_origins.rs:79`, `control_flow_operations.rs:323` and `target_lowering/references.rs:1546` (Grep over `source-rust/`).
- The audit counterpart of `lower_call` is `direct_calls::validate` (`legalization_target.rs:5016`); of `lower_no_normal` is `no_normal::validate` (258).

**Unused or one-caller code.** `retain_origin`: 1 caller (`control_flow_operations.rs:1297` via the `use` at 272). `resolve`: 4 call sites (the three above plus none in this file). `byte_arguments::shared_owner_view`: 1 call (1697). `owned_arguments::is_owned_parameter`: used in `control_flow.rs:2869, 2951` and here 1730, 1950.

**Stated limits.**
- "Only view formals can receive an array window. Resolving an argument's backing asks for its byte extent, which opaque loans need never reveal." (1662-1663).
- Error text "installation helper must have one structural formal" etc. are in `control_flow_operations.rs`.
- `OMEGA_COMPILER_CHECK_SITES` diagnostic at 349.

**Fail-open risks.** None seen. `land_result` for a published scalar call refuses a result value already defined (1324-1328). `.filter(...)`/`Option` misses produce `unsupported_control_flow` (for example 1141, 1156).

---

## 8. `04/src/target_lowering/control_flow_operations.rs` (1,562 lines)

**Purpose.** Dispatch of each non-terminator abstract operation to its lowering owner, plus lowering for accessor scalar access, float arithmetic, scalar arrays and scalar-source lookup. Header (1-9).

**Shape.**
- `accessor_scalar_access::lower` (25-109).
- `ieee_float_arithmetic::lower` (126-266): `Classify` branch (166-221) and scalar branch (222-265).
- `operations::lower_operation` (290-1300): one `match operation` of about 60 arms. First resolves a selected boundary call into an ordinary call (321-332), refuses any operation moving a borrowed leaf (340-342), runs the arm, then `retain_origin` (1296-1298).
- `scalar_arrays::shape` (1314), `establish` (1354).
- `scalar_sources::{ScalarSources 1400, source 1422, resolved_source 1430, runtime_indices 1472, projection_indices 1497, argument_indices 1550}`.

**Inputs/outputs.** Reads `RowSite` (function, types, `LiveDefinitions`, output `Vec<TargetUnitOperation>`, provenance), settlements, declarations. Pushes rows and updates `live` homes.

**How it decides.**
- `resolved_source` (1430-1470): looks up a `ValueId` in order: known integers, scalar homes, booleans, float constants, block parameters, function parameters; else `UnknownValue` (1463).
- `runtime_indices` (1472-1494): operand must be an integer with `bits() <= 64` per `types.closed_scalar(...)` (1481-1482).
- `BoundaryCall` dispatch (1005-1219) is keyed on `settlement.realization` variant: `EmptyRunRelease`, `StaticInterrupt{Install}`, other `StaticInterrupt`, `AccessLifecycle`, `AccessorScalarAccess`, `ProgramImageRetainedSuffixStart`, `NearestIeeeFloatArithmetic`, hosted/normalized foreign, else `Err` (1218).
- Static-interrupt installation: exactly two helper calls with fixed roles and argument slots (`Validate` at 0, `Work` at 5, 1045-1056); each helper must take one shared-borrow structural formal and no scalars (1062-1073).

**Cases by shape.**
- `EstablishActivationClaim`: `committed` must be `u64` (355-363).
- `scalar_arrays::shape` (1314-1352): nested `FixedArray` with `Literal` length ending in a `PrimitiveScalar`; loops at most `types.len()` times (1323). An empty array size is 0 (1337).
- `EstablishByteSequenceLiteral` (874-914): place kind and type shape must be `BorrowedView` and match the lookup, else `unsupported_control_flow` (881-895).
- `IntegerConstant` arm only for `ScalarType::Integer` (1241); other scalar types fall to `_ => Err` (1294).
- `ieee_float_arithmetic` Classify: declaration must have exactly `[operand]` scalar parameter and a `Structural` result matching type, multiplicity, qualifications (171-182).

**Same job elsewhere.** Lowering rows here have audit twins in `legalization_target.rs::unit::validate_operation` (5581-7299): for example `FloatResultPolicy` lowered at 604-659 and audited 5207-5251; `IeeeFloatCompare` 660-691 and 5253-5293; `ScalarRepresentationView` 692-724 and 5295-5330; `SelectMachine` 760-781 and 5936-5968; `SelectImplementation` 782-799 and 5969-5994; `ActivateTaskRegister` 443-475 and 5995-6038.
`FloatResultPolicy` inserts via `Arc::make_mut(...).insert(...)` and errors on duplicate (644-649); `IeeeFloatCompare` does the same insert but ignores the previous value (681, no duplicate check) while `ScalarRepresentationView` uses `insert_scalar_home` (712). Three different insertion paths for scalar homes (read).

**Unused or one-caller code.** `operations::lower_operation`: 1 call (`control_flow.rs:3255`). `scalar_sources::argument_indices` (1550): used at 432, 465, 513 in these files (read; not grepped).

**Stated limits.**
- "unreachable!("inline float arithmetic routes only boundary calls")" at 144: a panic, not an error.
- "Keep one row per source operation so the checked partition, its exact subject and its annotations survive without bytes." (826-827).
- "Retain selector geometry while the producer still owns the storage types." (1496).

**Fail-open risks.**
- `IeeeFloatCompare` (681) writes into `scalar_homes` without checking whether `result` was already defined (compare 644-649). Classification: traced, `control_flow.rs:2877-2919` already refuses duplicate defining values for `IeeeFloatCompare` (listed at 2886), so the duplicate cannot reach here.
- `ieee_float_arithmetic::lower` has `unreachable!` at 144; reachable only if the dispatcher sends a non-`BoundaryCall` there (dispatcher at 1156 sends only from the `NearestIeeeFloatArithmetic` boundary arm). Classification: traced.
- Otherwise none seen.

---

## Coverage

Read in full, in order:
- `04/src/preparation/legalization_target.rs` lines 1-7559 (7,559).
- `04/src/preparation/mod.rs` 1-226 (226).
- `04/src/preparation/publication.rs` 1-405 (405).
- `04/src/preparation/sum_layout_selection.rs` 1-4764 (4,764).
- `04/src/target_lowering/call_result_origins.rs` 1-184 (184).
- `04/src/target_lowering/control_flow.rs` 1-3463 (3,463).
- `04/src/target_lowering/control_flow_calls.rs` 1-2233 (2,233).
- `04/src/target_lowering/control_flow_operations.rs` 1-1562 (1,562).
Total read in full: 20,396 lines. Skimmed: 0. Not read: none.

Greps run (all with the Grep tool): `capture_source_publication`, `verify_source_publication`, `OMEGA_COMPILER_CHECK_SITES` (count), `runtime_indices_rejoin` (count), `call_result_origins`, `validate_external_completion_result|selected_boundary_calls::resolve|...retain_origin`, `dominance::schedule|SourceDominators|...` (files), `accepted_types|ModuleTypes` (count), `unsupported_control_flow` (count), `available: None|Checker \{`, `LegalizationError::custody\(\)` (count); Glob for `sum_layout_selection.expected.txt`.

## Five things a maintainer most needs to know about this unit

1. `legalization_target.rs` is a second implementation of the stage 04 lowering, written as a checker: for most rows the audit and the lowering encode the same rules twice (ProgramImageAddress, StaticInterrupt, ExternalCompletion, StructuralCase, calls). The audit runs only on an explicit validator request and in tests (`preparation/mod.rs:13-17`), so a lowering change that is not mirrored shows up only there.
2. Almost every mismatch in the audit is the same error value `LegalizationError::custody()` (186 uses in `legalization_target.rs`), and almost every refusal in lowering is `unsupported_control_flow` (152 uses across the three `control_flow*.rs` files). Neither carries the reason, so the failing rule is found by reading the code, not the error.
3. Lowering is a fixed-point loop over blocks (`control_flow.rs:3126-3344`) that swallows per-block errors as "deferred" and re-tries; the error surfaces only if the loop ends with a block still deferred (3338-3341).
4. Two dominator implementations live in this crate, one over bitsets (`control_flow.rs:135`) and one over reverse postorder with parent arrays (`legalization_target.rs:729`), and they treat a block outside the graph differently: lowering returns `None` (refuses), the audit's `dominates` returns `true` (822-824).
5. `sum_layout_selection.rs` is 4,764 lines of tests, not selection code. It compares against a checked-in text file (`sum_layout_selection.expected.txt`, 1092) and builds target rows by hand, so changes to row layouts need that file and these builders updated together.

## What I am unsure about

- Whether `sum_layout_selection.rs` has exactly 29 tests. I counted names while reading, not with a grep.
- Whether `capture_source_publication` and `verify_source_publication` agree for operations that have both a `cleanup_rosters` result and a `psi_edge` (for example `Crash` with cleanup actions). I read both functions but did not read `cleanup_rosters`.
- Whether `optimized_catalog_graph` returns `None` when the callback returns `None` for a node (`call_result_origins.rs:70-92`); that function is in `optimization_unit_semantics` and I did not read it.
- Who calls `validate_target` and `validate_call_descriptor_origins` (both in `preparation/legalization.rs`, outside the unit); stated from the test usage only.
- `control_flow.rs:2776` branch: when `reference_edge_exits` is empty `reference_arrival` returns the current state; I did not trace which terminators leave it empty beyond `Jump`.
- The other dominator implementations in `optimization-unit/src/dominance.rs` and `psi/semantics/src/control_graph/dominance.rs` were not read; "same job" is suspected from names.
