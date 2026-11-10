# Unit 57 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`, short names used below):
- `execution_terminal_unit_calls.rs` = calls.rs (1-9432)
- `execution_terminal_unit_cleanup.rs` = cleanup.rs (1-5292)
- `execution_terminal_unit_control.rs` = control.rs (1-6926)

All three are "module bundles": a short header comment, then many inline `mod x { ... }` blocks, then `pub(crate) use` re-exports at the end. All are stage 05 (typed trees -> checked trees); they build `Checked*` plan rows (calls, cleanup actions, call-occurrence accounting, boundary plans) from `CheckFacts` and the typed program (`&dyn validation::CheckingInput`). Almost every function returns `Option<_>` or `Result<_, &'static str>`; `None`/`Err` means "this body/call is not planned" (a plan gap), reported via `LocalConstructionTrace`.

---------------------------------------------------------------------------------------------------
## 1. calls.rs  (call closure, argument custody, claim transfer)

### Purpose
Turns one source call (a `FlowCallFact`) into one `CheckedUnitEffectOperationPlan` (usually `Call(CheckedUnitCallPlan)`): which target, which scalar/structural/erased arguments, what each structural argument lends (source + path + access), the result binding, and which linear claims transfer. Also holds the entry-signature classifier (`parameter_signatures`) and the `view_subslice` range admission. Header comment 1-14 lists the sub-owners.

### Shape (inline modules, with line spans, read in full)
- `affine_locals` 16-172: `build_affine_array_construction_prefix` (33).
- `argument_paths` 173-1497: scalar-argument selection (`checked_call_scalar_arguments` 230, `checked_call_erased_proof_arguments` 302, `checked_call_erased_scalar_arguments` 587), `ProjectedArgumentPath {declared_type, storage_type, path}` (617), `projected_argument_path` (911, the central place->path walker), wrappers `projected_borrowed_path` 634 / `projected_selected_path` 704 / `projected_observed_path` 752 / `projected_evaluated_path` 877, `borrowed_argument_endpoint` 657, `stored_reference_prefix` 727, `retain_local_reference_source` 827, `borrowed_index_segment` 1103, `value_index_segment` 1127, `fixed_byte_array_range_path` 1359, `open_byte_range_path` 1390, `byte_sequence_literal_argument` 1462.
- `argument_presentation` 1499-1615: `ArgumentPresentation {SameIdentity, SharedScalarObservation, ByteSequence, FixedArraySlice}` (1518) and `classify` (1541).
- `boundary_admission` 1616-1802: `boundary_trait_receiver_is_namespace` 1632, `boundary_authority_refusal` 1688, `fixed_array_slice_view_is_admitted` 1721, `fixed_byte_array_view_is_admitted` 1736, `fixed_array_slice_view_parts` 1760.
- `call_operations` 1803-2541: `ExpectedCallValueResult {Scalar, Structural}` 1842, `call_value_result_matches` 1855, `PlannedCall` 1913, `build_call_operation` 1931 (about 610 lines, the call entry point).
- `call_targets` 2542-2645: `ResolvedCallTarget {Requirement, DynamicSlot, Declared}` 2553, `resolve_call_target` 2575, `boundary_requirement` 2611.
- `computation_arguments` 2647-3568: `ComputationCallArguments` 2716 (`structural_argument` 2770, `subslice_argument` 2942), `borrowed_storage_argument` 3040, `borrowed_view_argument` 3260, `storage_argument` 3378, `primitive_local_argument` 3525.
- `dynamic_call` 3569-3830: `build` 3612, `dynamic_family_operands` 3775.
- `parameter_argument_paths` 3831-4309: `SourceParameter` 3872, `ArgumentCustody` 3915, `ArgumentPathShape` 3950, `ParameterArgument` 4007, `ordinary_parameter_path` 4020, `boundary_parameter_path` 4121.
- `reference_forwarding` 4310-4663: `preserves_mutable_referent` 4323 / `exact_mutable_referent` 4344 / `prefix_preserves_parameter` 4473 / `preceding_reborrow_preserves_carrier` 4567.
- `result_arguments` 4664-5904 (sub-mods `anonymous_shared` 4725, `bounded_owner` 4934, `reference` 5070): `argument` 5207, `argument_contract` 5256 (about 600 lines).
- `result_owners` 5905-5986: `structural_result_owner` 5918.
- `signatures` 5988-6274: `free_structural_scalar_signature*` 6021/6037/6052, `parameter_signatures` 6093, `entry_claims` 6198.
- `structural_arguments` 6275-7709: `structural_call_arguments` 6320, `structural_arguments_for_types` 6374 (about 550 lines), restored-alias translators 6967 / 7030, `exact_structural_argument_access` 7175, `exact_structural_borrow_access` 7239, `call_claim_transfers` 7317, `call_claim_events` 7418, `owner_claim_crossings` 7457, `claim_crossings_at_place` 7515, `claim_operand_path_matches` 7605, `computed_call_claim_transfers` 7660.
- `structural_result_call` 7711-7917: `build` 7737, `structural_custody_parts` 7855, `with_structural_custody` 7892.
- `trait_boundary_call` 7918-8457: `BoundaryRequirement {Trait, Operator}` 7965, `build` 7987 (about 470 lines).
- `view_subslice` 8458-9354: `ViewKind` 8504, `ViewRange` 8521, `ViewSubslice` 8545, `admit` 8640, `admit_resolved` 8684, `builtin_range_operation` 8749, `lends_collection` 8778, `admit_replayed` 8837, `retained_endpoints` 8880, `shape` 8948, `collection_storage` 9023, `whole_extent` 9168, `view_kind` 9277, `view_identity` 9311, `borrowed_view_access` 9332.
- Top level 9356-9432: `CallTarget` 9369 (`declared` 9390, `dynamic_slot` 9403) and re-exports.

### Inputs / outputs
Reads `CheckFacts` (`flow.control.calls`, `flow.ownership.permissions`, `borrow.*`, `values.scalar_expressions/scalar_computations/structural_values/proof_terms`, `contract_plans`, `service_reaches`, `requirement_call_specializations`, `nominal_machine_uses`), the typed program through `CheckingInput`, and a `ShapeCollector` (mutated: it enrols type identities). Produces `CheckedUnitEffectOperationPlan`, `CheckedUnitStructuralArgumentPlan {source, path, type_identity, access}`, `CheckedUnitClaimTransferPlan`, `CheckedUnitStructuralParameterPlan`, `CheckedUnitEntryClaimPlan`, `ViewSubslice`. Mutates `LocalConstructionTrace` (Cell-based, via `phase()` calls) as it goes.

### How it decides (keyed on what)
- Target selection is by symbol identity: `boundary_requirement` scans every trait's `executable_trait_machine_instances` and compares `signature.symbol == target` (calls.rs:2619-2623); two hits => `Ambiguous` => None (2642). `build_call_operation` special-cases builtin intrinsics by `builtin.name()` string through `from_intrinsic_name` (calls.rs:1982-1990 GDT, 2054-2062 registers): keyed on the intrinsic's source name.
- Argument admission is keyed on place identity + type identity: `base_type_identity(...) == target_identity` and `structural_access_for_type_reference`. `classify` (1541) compares `stored_structural_type_identity`/`base_type_identity` to the target identity, then falls through to carrier-specific rules.
- Access is keyed on the borrow ledger: `exact_structural_borrow_access` (7239) finds exactly one `BorrowCallFact` for (statement_index, call_ordinal, target_symbol) and requires all accesses at the same (root, segments) to have one kind.
- Claim transfer is keyed on permission-event identity: `call_claim_events` (7418) filters events by source `Call{statement_index,call_ordinal,target_symbol}`, kind Transfer/Consume, Owned, Linear, `obligation_live`; final check `output.len() == events.len()` (7403, 7707).
- Result custody: `call_value_result_matches` (1855) compares return-type identity and multiplicity.
- Dynamic call requires the first scalar param to be `PrimitiveType::Selector` at position 0 (3678-3684).
- No display-name or `format!` keyed decisions found in calls.rs, except diagnostics/trace text (`eprintln!` at 5742, 6911; `format!` is not used for decisions). `reborrow_restored_shared_cohort_observation_alias_target` (7030) keys on call_ordinal == 0 and on the retained cohort size (see 5).

### Cases by shape (fixed lists that refuse the rest)
- "3..=26" array length for the affine array construction prefix: `.filter(|length| (3..=26).contains(length))` (calls.rs:48-51); also element must be an empty record, mutable local with no initializer, assignments exactly `arr[i] = Elem{}` in order (111-132).
- Runtime-index selectors: constant immutable local (1178), bare scalar parameter of U8..I64 (1210, list at 1223-1232), or integer field read under a structural entry parameter (1253, list 1276-1287). Everything else returns None. `PlaceSegment::OpaqueRange` is rejected (1011).
- `ArgumentPathShape::Static { fields: 0, fixed_indexes: 1..=3 }` at calls.rs:4082-4086 (count-keyed admission row) in `ordinary_parameter_path`; the table is ordered "first admitting row decides" (3834-3836).
- Restored-alias cohort: "exact two- or three-child shared-freeze cohort" (calls.rs:7023-7029); `matches!(target_parameters.len(), 2 | 3)` (7051) and requires `call.call_ordinal == 0` (7041) and the callee body to have zero statements (7047-7049).
- `fixed_array_slice_view_parts` (1760): literal fixed array -> `&[T]` / `&mut [T]` only. `fixed_byte_array_view_is_admitted` (1736): element must be `Named` and `U8`.
- `byte_sequence_literal_argument` (1462): only `BorrowedView`+shared or `BoundedOwned`(capacity) + Unrestricted.
- `trait_boundary_call::build` (7987) "callback telescope" rules: `callback_parameters.len() <= callback_machine_parameters.len()` (8141) and one-to-one binder name match (8142-8148).
- `view_subslice`: `ViewKind {Bytes, Elements}` only (8504); `domain_qualified_byte_view` allows only `Domain` constraints (9226-9274).
- `is_closed_primitive_array_type` parameters: only whole-owned-unrestricted pass-through (calls.rs:6776-6786).

### Same job elsewhere
- "Pick a retained computation root, else a bound pure expression, check its primitive type, wrap as `CheckedCallScalarArgument::{Computation,Pure}`": done in `checked_call_scalar_arguments` (calls.rs:230-294), `checked_call_erased_scalar_arguments` (587-610), `dynamic_family_operands` (3807-3827, Pure only), `scalar_arrays::elements` (control.rs:5533-5620), `retained_endpoints` (calls.rs:8898-8930). Grepped `bound_expression_at(`: 21 hits in 7 files in this crate (calls.rs 4, control.rs 1, execution_terminal_unit.rs 3, statement_sequence 3, state_graph 2, structural_scalar_store 6, scalar_computations 2). Suspected further duplicates in those files (not read).
- Hook/claim "Unknown => refuse; collect events; compare count to event list" appears in calls.rs:4931 (`covered == events.len()`), 6272 (`output.len() == events.len()`), 7403, 7707.
- The "erased/non-erased parameter classification, scalar vs structural" loop appears in `build_call_operation` (calls.rs:2200-2219), `dynamic_call::build` (3661-3677), `trait_boundary_call::build` (8168-8183) and `parameter_signatures` (6103-6195). `parameter_signatures` is documented as the one classifier for state, slot, bodyless boundary and requirement (6084-6092), yet the first three re-implement a scalar-position loop (read).
- `structural_arguments_for_types` (6374) and `trait_boundary_call::build` (7987) each contain their own arm sequence (byte literal -> subslice -> caller structural result -> primitive local -> borrowed storage -> parameter path): same job, two copies. The first also contains view-local, trivial-affine-local and retained-temporary arms the second lacks (read, 6410-6848 vs 8168-8338).
- `borrow_access_spelling`/`exact_structural_argument_access` in calls.rs vs `lends_collection` (8778) both walk `borrow.loans` matching statement and kind; different tables.
- Local selector-resolution closure (`CallSelector` root -> `ComputedIndex`) is written out three times: calls.rs:6443-6454, 8231-8242 (boundary), and 7624-7639 (`claim_operand_path_matches`).

### Unused / one-caller code (greps)
- `ArgumentPresentation` / `argument_presentation` module: 8 occurrences of "argument_presentation" in calls.rs and 2 in state_graph.rs (grep count, stage-05 `src`). The header says it "owns shared carrier compatibility" (calls.rs:4) and `classify` is "Carrier presentation shared by ordinary calls, boundary calls and state edges" (1501): users outside calls.rs: state_graph.rs 2 hits. So one outside file (state edges) as claimed; inside calls.rs about 6 uses.
- `fixed_byte_array_view_is_admitted | fixed_array_slice_view_is_admitted`: 10 hits, all in calls.rs (grep, 1 file). No outside users.
- `view_subslice::(admit|admit_resolved|admit_replayed|shape|whole_extent|retained_endpoints|builtin_range_operation)`: 10 hits in 5 files (state_graph 1, statement_sequence 2, calls 3, execution_terminal_unit 1, scalar_computations 3). Doc comment 8458-8475 says "Local bindings, calls and state transfers carry the same range operation", consistent.
- `projected_observed_path|borrowed_index_segment|retain_local_reference_source|stored_reference_prefix|projected_selected_path`: 25 hits in 5 files (values.rs 1, structural_scalar_store 3, calls 11, execution_terminal_unit 4, scalar_computations 6).
- `structural_result_owner`: doc says "Consumers share this query instead of rebuilding a partial producer list" (calls.rs:5906-5910). Users outside calls.rs: execution_terminal_unit.rs 2 call/use sites (3841, 6943; plus use at 6493) and state_graph.rs 1 call (5995, use 5762). Three callers in total.
- `byte_sequence_literal_argument`, `boundary_authority_refusal`, `open_byte_range_path`, `fixed_byte_array_range_path`, `parameter_signatures`, `free_structural_scalar_entry_signature_traced`, `boundary_trait_receiver_is_namespace` combined grep: 30 hits across 6 files; individually not counted.
- `ComputationCallArguments::subslice_argument` (calls.rs:2942) and `ViewSubslice::transfer` (8631): not separately grepped (suspected few callers).
- `ResolvedCallTarget::identity` used at 2155 and 3727.
- `call_claim_events` is private and used at 7327 and 7669 only (read).

### Stated limits
- "This is intentionally not a general local-alias resolver." (calls.rs:6965).
- "A forwarded in-scope binder or a missing row leaves the requirement unplanned." (control.rs:1160, same idea) / calls.rs 8349-8358 "unadmitted boundary executable telescope".
- "Runtime endpoints need separate extent evidence; this route retains only canonical fixed bounds" (calls.rs:1357-1358).
- "A slice carries no declared extent ... the hop stays unproven" (calls.rs:1305-1309): index into slice accepted without a static bound, left to Terminal.
- "An interior range, which no argument path spells." (calls.rs:3965).
- Erased/mutable formals on selector calls: "erased or mutable owned scalar formals retain their existing proof-lane coverage boundaries" (calls.rs:3583-3585).

### Fail-open risks
- `fixed_index_within_extent` for a slice container returns `Some(())` without checking the index (calls.rs:1311-1320, `slice_type_reference(...).then_some(())`). Stated: "the hop stays unproven exactly the way an emitted segment's Terminal obligation re-proves it" (1305-1309). Wrong program that gets through here: a constant out-of-range index on a slice. Closed by Terminal per the comment (not traced).
- `projected_argument_path` runtime Index segment carries no bound; "Terminal re-proves `0 <= index < extent`" (calls.rs:627-629, 1206-1209). Stated.
- `exact_structural_borrow_access` returns `Owned` immediately for owned targets (calls.rs:7249-7251): no borrow ledger consulted; ownership is checked by `call_claim_transfers`. Traced to that function only.
- `result_arguments::argument_contract` for a projected mutable borrow rooted at an expression returns `StructuralResult` without `exact_structural_argument_access` (calls.rs:5678-5691); open (not traced to its callers' other checks).
- `claim_crossings_at_place` returns an empty Vec when no event matches (7542-7544), then `call_claim_transfers` relies on the final count equality (7403) to catch an uncovered event; a transfer with no matching event is not itself an error. Traced inside the file.
- `anonymous_shared::validate`: `if multiplicity == Unrestricted { return Some(()) }` (4837-4839) skips the permission-event join for unrestricted owners. Stated by comment 4726-4735 only for the shared-temporary design; no comment at the early return.
- `structural_arguments_for_types`: `exact_structural_argument_access` failing yields None; but `restored_alias.is_some()` path sets `access: target_access` with no access check (6823-6825). Stated: certificate (6962-6966 "the one bare reference carrier admitted by the checked post-reactivation certificate").
- `signatures::parameter_signatures` `additional_input_qualifications.retain(...)` drops qualification rows whose domain is unknown (returns false when no domain matches, 6172-6174), i.e. fail-closed by dropping. Note: dropped rows are removed from the plan rather than rejected; open.

---------------------------------------------------------------------------------------------------
## 2. cleanup.rs  (nominal and partial-affine Unit cleanup planning)

### Purpose
Turns the checker's "contextual cleanup dispositions" (who dies at a state exit, call continuation, replacement, or value completion) into ordered `CheckedNormalCleanupAction`s (`Discard`, `InvokeNominal`, `RetireHome`) plus partition trees (which regions are discarded or hook-invoked under which case conditions), and installs them into plans. Also builds the "nominal affine unit cleanup machine" plan for tiny `dropped by` hooks. Header comment 1-15.

### Shape (read in full)
- `mod actions` 17-992: `DispositionActions` 41, `home_retirement` 62, `finish_home` 98, `selected_region` 123, `disposition_actions` 187, `disposition_actions_at` 210 (about 400 lines, core), `retain_partition` 612, `checked_case_segments` 771, `checked_path_segments` 789, `selected_nominal` 897, `consumed_claims` 929.
- `mod anonymous` 994-1387: `projected_temporary_cleanup` 1025, `insert_continuation` 1062, `normal_call_completion` 1101.
- `mod cleanup_evidence` 1389-1517: `machine_has_content_evidence` 1399, `has_exact_root_affine_discard` 1418, `has_exact_symbol_affine_discard` 1433, `has_exact_root_nominal_disposition` 1471.
- `mod descriptor_retirement` 1518-1554: `descriptor_retirement` 1527.
- `mod nominal_requirements` 1556-1819: boolean-field `requires` handling (1570-1818); `direct_boolean_field_requirement` 1750.
- `mod normal_edges` 1820-2746 (sub-mod `exits` 1827): `state_exits` 1845, `state_exit_storage` 1865, `state_exit_rows` 1899, `completed_body_outcome` 2001, `state_cleanups` 2046, `sequence_cleanups` 2108, `cleanup_rows` 2176, `normal_result_tree` 2322, `cleanup_rows_at` 2456, `owner_source` 2642, `expression_source` 2669, `producer_root` 2708.
- `mod order` 2747-2794 (`merge` 2756), `mod partial_affine_cleanup` 2795-2821, `mod replacements` 2822-3259 (`actions` 2847, ~410 lines), `mod residuals` 3260-3300, `mod retained_owners` 3301-3444, `mod temporary_actions` 3445-4895 (tests 3467-3595; `OperationSequence` 3707; `OperationSelections` 3755; `operations` 3802; `sequence_actions` 3829; `value_call_actions` 3986; `call_actions` 4031; `install` 4204; `value_call_context` 4285; `finalize_completion_actions` 4345 (~435 lines); `finalize_value_completions` 4787), `mod temporary_descriptors` 4896-5009 (`completions` 4912).
- Top-level 5010-5292: `owning_cleanup_machine` 5039, `callable_target` 5059, `build_nominal_affine_unit_cleanup_machine` 5075, `nominal_cleanup_shape` 5222, `nominal_cleanup_target_matches` 5248.

### Inputs / outputs
Reads `facts.contextual_cleanup` (dispositions, exclusions, segments, selection), `facts.flow.ownership` (permissions, segments, selection transfers), `facts.flow.control` (outgoing, exits, calls, operands), `facts.values.*`, and the callable catalog `targets: &[CheckedUnitCallableEntry]`. Mutates `ShapeCollector.cleanup_control` (arena tables: partitions, partition_nodes, actions, conditions, paths, normal_exit_rows, ...) and, at finalize, `facts.values.scalar_computations` and `facts.flow.terminal_unit_effects` (cleanup_control, structural types).

### How it decides
- Whether a disposition applies: matched on symbol/identity: `disposition.source == PermissionEventSource::StateExit` (cleanup.rs:2501), `disposition.exit == selected` (2507), `disposition.root`/`machine`/`state` (3049-3062), call coordinate `(statement_index, call_ordinal, target_symbol)` (3339-3341, 4115-4121).
- Hook selection: `crate::flow::selected_cleanup_leaves_in` (251), then `selected_nominal` (897) which requires a unique callable target (`callable_target` 5059) and `nominal_cleanup_target_matches` (5248): receiver is `&mut`/write-only reference to the exact type identity, no type params, no owned data, no trait conformances, no invokes, not suspends/blocks, Unit return, <= 1 structural parameter at position 0.
- Case reachability: `case_evidence.reachability(...)` => `Inactive` skips, `Selected` proceeds, `Runtime` requires "selected transfer custody" unless cause is `Unestablished` (cleanup.rs:337-350).
- Action ordering: `order::merge` sorts by `crate::flow::cleanup_path_order` (2784-2788).
- Nominal requirements: `direct_boolean_field_requirement` (1750) accepts only `x.f`, `!x.f`, `x.f == true/false`, `x.f != true/false` (1758-1795) where `x` is the receiver parameter and `f` a one-segment field whose symbol appears in ANY data definition with `PrimitiveType::Bool` type (1802-1811: `program.data_definitions().iter().any(...)` keyed on `field.symbol` only, not on the receiver's data type).
- Name use: `incomplete_lifetime_capture_diagnostic` (3602) builds debug text with `format!("{:?}")` for diagnostics only; `nominal_cleanup_missing_requirement_diagnostic` (1692) builds an error string using `machine_name`/`symbol_name` labels (message only).
- `incomplete` custody uses `program.symbols.name(...)` in `refusal` strings only (4390-4401).

### Cases by shape
- `build_nominal_affine_unit_cleanup_machine` (5075) admits only: exactly one state; no type params/owned data/conformances/invokes; not suspends/blocks; Unit return; empty body (`state_statements(...).is_empty()` 5094); at least one parameter; every parameter owned, non-const, non-mutable, Affine or Linear, no qualifications, non-generic closed data, and `is_bounded_nominal_cleanup_record(shape)` (5109-5129).
- `partition_nodes`: Scope / Leaf / Cases only (retain_partition 632-694).
- `ReleaseReference` completions: "two- or three-child" does not appear here.
- `cleanup_rows` scalar-return case: `key()` maps exit kinds FinalExpression/TransitionTarget/TransitionContinuation to (stmt, 0/1/2) and errs for other kinds (2205-2214).
- `temporary_descriptors::completions` retires only affine, unnamed (`!binding.local.symbol.is_valid()`), descriptor-cleanup result bindings (4948, 4965).

### Same job elsewhere
- `has_exact_symbol_affine_discard` (1433) and `affine_locals::build_affine_array_construction_prefix`'s exit-event filter (calls.rs:150-169) do the same event match (AffineDrop, StateExit, Owned, Affine, `claim_identity == Unknown`, `!obligation_live`, root symbol, empty segments): same job, two copies; the calls.rs copy additionally requires a provenance `Established{statement_index:0}` and the cleanup.rs copy takes provenance as a parameter.
- Disposition source-coordinate matching `PermissionEventSource::Call{statement_index, call_ordinal, ..}` repeated in cleanup.rs 1184-1191, 2501, 3339, 4115, 4153, 4572-4579 and in calls.rs 6702-6707, 7431-7436.
- `selected_region` (123) and `disposition_actions_at` (210) each recompute `statement_index`/`reference`/`path` from a disposition (lines 133-173 vs 231-294): same job.
- The "match a producer by statement and call ordinal in flow calls, require exactly one" idiom: cleanup.rs 1236-1248, 2723-2738, 4466-4472 and control.rs several (e.g. 4161). Suspected shared helper elsewhere: `facts.flow.control.call_at(...)` is used at 4140 (cleanup.rs).

### Unused / one-caller code (greps)
- `descriptor_retirement\(|partial_affine_residuals|nominal_cleanup_shape|finalize_completion_actions|projected_temporary_cleanup|is_bounded_nominal_cleanup_record` combined count: 41 hits in 18 files (grep). Of these, in cleanup.rs: 12.
- `state_exits|state_exit_storage|machine_has_content_evidence|has_exact_symbol_affine_discard|has_exact_root_affine_discard|checked_contract_expressions|direct_boolean_field_requirement` combined: 23 hits in 5 files (cleanup 16, execution_terminal_unit 2, published_facts 2, state_graph 1, statement_sequence 2).
- `structural_result_owner` is defined in calls.rs, not cleanup.rs (see above), but `producer_root` (cleanup.rs:2708) does the equivalent producer->root join and `expression_source` (2669) the inverse; both used in this file (2098/2144 and 4100).
- `ReleaseReference`'s `owner_cleanups` (3790-3793, 3847-3883) has two readers in this file (`release_owned_dispositions` 3781, `sequence_actions`).
- Test module at 3467-3595 tests only `value_call_context` (3 tests).

### Stated limits
- "Lifetime binders retain checked loan relations but add no runtime parameters" (cleanup.rs:12-14).
- "does not reconstruct a second permission partition or dispose a selected live linear obligation" (1007-1009).
- `descriptor_retirement` returns `None` for Linear (1551) with no comment at that arm.
- Replacement: "a displaced old value already has a selected disposition" => Err (3099-3101); "cleanup region partition did not select every hook" => Err (597-599).
- Residuals doc (3261-3272): "Membership and nominal-hook admission remain the caller's responsibility."
- "A touched case instead retains only its selected payload siblings and common fields" (3270).

### Fail-open risks
- `consumed_claims` (929-991) returns `Some(empty)` when no consume event matches the path (990); a hook with no linear claim events is accepted with `consumed_claims: []`. Wrong program that gets through: a hook on a path whose linear claim was never recorded would not be refused here. Open; the final cleanup requirement check is elsewhere (not read).
- `has_exact_root_nominal_disposition` (1471) for Affine returns `claims.is_empty() && has_exact_root_affine_discard`; Linear requires exactly one claim. No accept-on-missing.
- `normal_edges::cleanup_rows_at` `source_for_owner(...)` returning `Some(None)` => `continue` (2549-2551): a disposition whose owner has no retained structural storage is skipped silently (no action). Traced: the `sequence_cleanups` closure returns `Some(None)` for a parameter with no structural plan (2158-2164), i.e. a scalar parameter; for structural roots it errors. Reasonable but unstated.
- `completed_body_outcome` (2001): "Unknown source outcomes remain potentially normal" (1998-1999): `normal_completion_possible != Some(false)` => `Possible`. Stated.
- `direct_boolean_field_requirement` field check keyed on field symbol in any data definition (1803-1811): accepts a `requires x.f` where `f` is a Bool field of any type if `place_at` resolves root to the receiver and one Field segment; since the symbol is unique per field this is equivalent. Traced: not a fail-open.
- `finalize_completion_actions` `owners` loop: if `reserved` operand is only a descriptor (`reference_shell`) it adds `descriptor_retirement`; if the selection is empty the operand check at 4683-4694 errors. Fail-closed.

---------------------------------------------------------------------------------------------------
## 3. control.rs  (structural control and boundary-machine construction)

### Purpose
(a) Builds `CheckedBoundaryMachinePlan`s for bodyless/boundary declarations, trait requirements, operators and dynamic slots; (b) accounts for which flow `FlowCallFact`s each statement/call owns so no call is executed twice or dropped (`call_occurrences`); (c) binds call results to locals (`call_results`); (d) builds one checked machine (`checked_machine`); (e) the `LocalConstructionTrace` that records where a builder stopped; (f) structural operand planning (`structural_operands`). Header 1-9; statement_sequence is a separate file included by `#[path]` at 5850.

### Shape (read in full)
- `boundary_contracts` 11-234: `boundary_scalar_requires` 26, `pending_recipe_requires` 193.
- `boundary_machine` 235-1305: `boundary_execution` 268, `build_boundary_machine` 296, `build_dynamic_slot_plans` 466, `machine_gap` 584, `build_static_boundary_requirements` 599, `build_boundary_operator` 891, `reference_result_sources` 993, `reference_outputs` 1019, `reference_output_path` 1124, `specialized_signature_substitutions` 1164, `boundary_result_plan` 1262.
- `call_occurrences` 1306-4544, sub-mods: `collecting` 1432-2676 (`collect_assignment_calls` 1454, `collect_constructor_calls` 1543, `statement_computation_prefix` 1645, `call_owns_statement_computations` 1745, `collect_argument_calls` 2117, `collect_range_calls` 2270, `collect` 2405), `dispatch_producers` 2677-3122, `selected_calls` 3123-3489 (`SelectedInvocationAccounting` 3136), `structural_values` 3490-3775, `traced` 3776-4329 (`outer_calls_in_prefix_traced` 3806); then `terminal_invocation` 4335, `completion_expression` 4377, `tail_call` 4402, `unit_statement_call` 4428, `ordered_statement_call` 4452, `outer_calls_traced` 4490, `is_terminal_call_occurrence` 4513.
- `call_results` 4545-4852: `bind_scalar_call_result` 4573, `bind_structural_call_result` 4642, `checked_copied_result_type` 4726, `checked_structural_result_type` 4772.
- `checked_machine` 4853-5230: `build_checked_machine_residual_parts` 4885 (~345 lines).
- `construction_trace` 5231-5445: `LocalConstructionTrace` 5251, `DeferredFailure` 5297.
- `reference_captures` 5446-5523 (`plan` 5454), `scalar_arrays` 5524-5650 (`elements` 5533, `assignment_type` 5627), `selected_copies` 5651-5849 (`SelectedCopies` 5660), `statement_sequence` (path include, 5850), `structural_operands` 5852-6905 (`Operand` 5891, `evaluated_argument_root` 5921, `value_calls` 6107, `establish_value` 6429, `for_call` 6475, `collect` 6516, `result` 6654, `plan_call_operands` 6712).
- Re-exports 6907-6926.

### Inputs / outputs
Reads `CheckFacts` (flow.control calls/operands/outgoing/exits/storage snapshots, values.scalar_computations/structural_values/completions, contract_plans, service_reaches, nominal_machine_uses, requirement_call_specializations, operators) and the typed program. Produces `CheckedBoundaryMachinePlan`, `CheckedDynamicSlotPlan`, `CheckedUnitEffectMachinePlan` (+ residual discards), `CheckedStructuralValueCall`, `SelectedCopies` rows; mutates `ShapeCollector`, a `CallOccurrenceLedger`, `LocalConstructionTrace`.

### How it decides
- Occurrence accounting is keyed on identity: arena `Handle<FlowCallFact>` keys `(arena_index, generation)` (control.rs:1337, 2687-2692), `(statement_index, call_ordinal)` coordinates (2802, 4065), `std::ptr::eq` on call references (1758, 1885, 4090). Duplicate `(statement_index, call_ordinal)` => None (4066-4071).
- `traced::outer_calls_in_prefix_traced` refuses if two outer calls share a statement ("duplicate statement call", 4095-4109) and if any in-prefix call is unclaimed (4225-4266).
- Boundary execution selection: `AccessExecutionDeclaration::authenticate` / `EmptyRunReleaseDeclaration::authenticate` on the machine symbol (268-293); no name matching: "An application with the same name or shape never acquires a core route" (264-267).
- Boundary requirement specialization: `specialized_signature_substitutions` (1164) requires every flow call to the requirement to have a `requirement_call_specializations` row; `Type` binders need a binding, `Machine` binders a valid selection equal to the authored static machine argument at that ordinal (1223-1246); differing bindings across calls => None (1247-1251).
- `checked_structural_result_type` (4772) classifies by type shape: stored loan, borrowed slice, bounded byte buffer, else refuses unit/primitive/reference and (unless Linear) requires `BorrowFree | closed primitive array | reference record | OwnedOrSharedView` contents (4818-4838).
- `evaluated_argument_root` (5921): per-formal-shape match (reference with stored loan / slice view / completed copy / transported / formation / shared referent) before looking up the structural value root and comparing normalized identity (6029-6031).

### Cases by shape
- `build_checked_machine_residual_parts` (4885): "single-state body" only (4900); return type must be Unit, or one of a list of reference/slice/view/owned/array/linear/primitive shapes (4904-4952), else None; scalar result needs `scalar_result_contract_is_represented` (4959-4963).
- `outer_calls_in_prefix_traced` statement-kind list with trace phases for Assignment/LocalData/Call/Expression/Transition/Assembly/AssemblyFact (4240-4262).
- `terminal_invocation` (4335): last statement only, transition with `Always` guard, no continuation, `TransitionExit::Ordinary`, no earlier transition (4345-4363).
- `tail_call` (4402) only the final statement of a Unit-returning state.
- `call_owns_statement_computations`: `scheduled_dispatch_subject`, `scheduled_by_active_arm`, `other_transition_arm`, builtin functions with `has_empty_write_frame()` (1888-1894).
- `operand_value_nests_calls` list of structural value kinds (1952-1967).
- `boundary_scalar_requires` projects memberships with a node budget `remaining = 4096` (74) and refuses `ProofFact::Proposition` (165).
- `reference_captures::plan` refuses mutable/erased locals and references with selectors (5462-5474), and a `StructuralExpression` source (5515-5520); selectors only `CheckedScalarExpression::Local` (5486).

### Same job elsewhere
- Scalar-root-or-pure selection duplicated: control.rs:5573-5620 (`scalar_arrays::elements`) vs calls.rs:252-291 (`checked_call_scalar_arguments`), 8898-8930 (`retained_endpoints`), 3812-3827.
- `statement_computation_prefix` (1645) and `collect_constructor_calls` (1543) both walk `roots_for_statement` with role filters; `collect_assignment_calls` (1454) a third walk; `traced::outer_calls_in_prefix_traced` (3806) re-walks the same roles (3917-4044) per statement.
- `value_calls` (6107) call-binding block (6178-6290) and `plan_call_operands` call block (6836-6901) both: `result(...)`/`checked_structural_result_type` -> `build_call_operation(..Structural(&result)..)` -> `bind_structural_call_result` -> `normal_call_completion`. Same job, two copies.
- `boundary_machine::build_static_boundary_requirements` (599) trait loop and `build_boundary_operator` (891) and `build_boundary_machine` (296) each assemble `CheckedBoundaryMachinePlan` with the same field list (820-869, 947-986, 416-452).
- Callback-telescope check (`callback_telescope` 657-678) vs the same logic in `trait_boundary_call::build` (calls.rs:8101-8164): same job, two copies with different strictness (the calls.rs one uses `nominal_use_at_binder` with site + `callback_entry`, the control.rs one `nominal_use_backs_binder` without site).

### Unused / one-caller code (greps)
- `bound_expression_at(`: 21 hits in 7 files (see section 1).
- `boundary_authority_refusal` is called at control.rs:4102 (traced) and calls.rs:8386 (grep of the combined name list: 30 hits in 6 files, control.rs 5).
- `checked_copied_result_type` (4726): not grepped separately; only `pub(crate)` defined here and not re-exported at 6914-6916 (only `bind_scalar_call_result`, `bind_structural_call_result`, `checked_structural_result_type` are). Suspected unused or used only by path from another file.
- `selected_copies::SelectedCopies::for_destinations`/`for_destination`: not grepped.
- `DeferredFailure`: pub(crate) at 5297; users not grepped.
- `LocalConstructionTrace::result_custody_failure` (5392): not grepped.
- `dispatch_producers` has a test (3057-3121) asserting the prefix owner index matches a linear join: an indexed structure shadowing a linear scan.

### Stated limits
- "`None` means some operand, receiver, or contract has no admitted boundary presentation; selection never weakens that signature." (calls.rs:7983-7985).
- "A forwarded in-scope binder or a missing row leaves the requirement unplanned." (control.rs:1160-1162).
- "A statement mark without a state belongs to the single-state builder's only state." (5398-5399).
- "The trace is diagnostic only: it never changes which bodies are admitted." (5240-5241).
- `boundary_contracts::pending_recipe_requires` doc: "these rows grant no proof" (191).
- `completion` in `value_calls`: if completion is None returns None with a trace (6408-6418).

### Fail-open risks
- `boundary_scalar_requires` (26-187): when a membership value is not a parameter, it is projected via `lower_callable_scalar_membership` and failure => `?` => None (fail-closed). When `structural && scalar_membership_value_carrier(...).is_none()` it `continue`s (109-114): a structural-lane membership with no scalar carrier is skipped from scalar requires. Stated by the doc: "Only an exact structural qualification restatement belongs to the structural lane" (20-24).
- `build_boundary_machine` falls back to `pending_recipe_requires` when scalar requires don't project (366-378) and records the raw clauses as `pending_scalar_requires`; "Final rebuilding must lower every clause normally; these rows grant no proof" (190-191). Stated. Whether the later rebuild refuses unknowns is not traced.
- `reference_outputs` skipped (`Vec::new()`) when `checked_family || preserves_descriptors` (398-402): boundary declarations for a family whose members all supply checked bodies, or an interrupt that preserves descriptors, publish no reference outputs. Open.
- `collecting::collect` (2405-2675): `ComputationKind::RetainedConstant | Value(_)` => no calls (2504); fine. `ByteSequencePredicate`: if there is no occurrence of the source call, nothing is recorded and Some(()) is returned (2546-2563: `if let Some(..) = occurrences.next()`). A predicate with no flow call is accepted. Open.
- `statement_computation_prefix` Guard root absent => no guard calls recorded (1670): `if let Some(root) = roots.next()` (no else). Open.
- `call_owns_statement_computations` final `.all(...)` accepts any remaining statement call that is retired, a builtin with `has_empty_write_frame()`, or on another transition arm (1881-1898). Wrong program that would get through: a call marked retired or in another arm that was actually unscheduled; reliance on `is_retired`/arm classification (not traced).
- `selected_copies::SelectedCopies::new` skips primitive non-reference copies (`continue`, 5695-5699): scalar copied payloads carry no copy row. Stated indirectly (scalars flow through scalar plans).
- `plan_call_operands`: `Operand::Storage` with `windows.move_call_operand` returning None => `continue` (6767-6768, `Operand::Storage {..} => continue`): a storage operand with no window move is silently not planned here; `build_call_operation` later must find it. Traced partially (the call build will fail if the argument is not found).

---------------------------------------------------------------------------------------------------
## Greps run (all with the Grep tool, path = omega-rust unless noted)
- `argument_presentation` in stage-05 src: calls.rs 8, state_graph.rs 2.
- `structural_result_owner` in stage-05 src: execution_terminal_unit.rs 3, state_graph.rs 2, calls.rs 2 (7 total).
- `projected_observed_path|borrowed_index_segment|retain_local_reference_source|stored_reference_prefix|projected_selected_path`: 25 in 5 files.
- `view_subslice::(admit|admit_resolved|admit_replayed|shape|whole_extent|retained_endpoints|builtin_range_operation)`: 10 in 5 files.
- `fixed_byte_array_view_is_admitted|fixed_array_slice_view_is_admitted`: 10 hits, all calls.rs.
- `descriptor_retirement\(|partial_affine_residuals|nominal_cleanup_shape|finalize_completion_actions|projected_temporary_cleanup|is_bounded_nominal_cleanup_record`: 41 in 18 files.
- `bound_expression_at\(` in stage-05 src: 21 in 7 files.
- `fn reference_formation_source|saved_dispatch_call|names_whole_local|constant_local_index|fixed_array_literal_length` across psi/compilation: `reference_formation_source` validation/src/recasts.rs:5185; `saved_dispatch_call` validation/src/places.rs:3608; `names_whole_local` only calls.rs:5863; `constant_local_index` only calls.rs:1178; `fixed_array_literal_length` only calls.rs:1338. No same-named duplicates outside calls.rs.
- The very broad `classify\(` count (141 across 57 files) is noise and not used.

---------------------------------------------------------------------------------------------------
## Coverage
- calls.rs: lines 1-9432 read in full (9432).
- cleanup.rs: lines 1-5292 read in full (5292).
- control.rs: lines 1-6926 read in full (6926).
- Skimmed: none intended. The long bodies (`build_call_operation`, `argument_contract`, `disposition_actions_at`, `finalize_completion_actions`, `collect`) were read at one pass; function-level judgments above are from that single pass, not re-traced.
- Not read: none. Total 21650 lines.
- Not done: callers outside this unit were mostly not read; "suspected" marks are explicit above. Counts above are from the greps listed, run once each.

## Five things a maintainer most needs to know about this unit
1. These three files do not make final decisions about program validity so much as decide whether a plan row can be built: nearly every guard returns `None`/`Err`, and the trace (control.rs:5251-5445) is the only record of where the builder stopped. A fail-closed refusal here shows up as a "plan gap", not a diagnostic.
2. Argument admission is an ordered table of special shapes (calls.rs:4020-4112 `ordinary_parameter_path`, 6410-6848 `structural_arguments_for_types`, 7987-8338 `trait_boundary_call::build`, 5256-5856 `result_arguments::argument_contract`). Ordinary-call and boundary-call paths each carry their own arm sequence; changing one does not change the other.
3. Claim transfers and cleanup both finish with count-equality against the permission-event list (calls.rs:7403/7707, 6272, 4931; cleanup.rs `selected_hooks != hooks.len()` 597). A missing or extra event is the usual refusal; an event with no matching argument yields None.
4. Fixed-count special cases exist: affine array prefix 3..=26 (calls.rs:48-51), fixed index count `1..=3` (calls.rs:4084), two/three-child shared-freeze cohort (calls.rs:7041-7056, require `call_ordinal == 0` and empty callee body), single-state body (control.rs:4900), single-state, empty-body nominal hook machine (cleanup.rs:5083-5129).
5. `view_subslice` (calls.rs:8458-9354) is the shared range admission, used by 5 files (grep); `ViewKind`/`ViewStorage` split Bytes/Elements and Established/FixedArray, with `Parameter`/`OwnedLocal`/`ViewLocal` roots (9044-9102).

## What I am unsure about
- Whether late checks elsewhere (Terminal verifier, lowering in stage 06) cover the fail-open spots listed (slice index bound, `pending_scalar_requires`, `consumed_claims` empty, `ByteSequencePredicate` with no flow call, missing Guard root). I did not read those sites.
- The duplicated-arm claims (`structural_arguments_for_types` vs `trait_boundary_call::build`, `value_calls` vs `plan_call_operands`) are from reading both blocks once; I did not diff them line by line.
- `checked_copied_result_type`, `SelectedCopies::for_destinations`, `DeferredFailure`, `ViewSubslice::transfer`, `ComputationCallArguments::subslice_argument` were not grepped for call-site counts.
- The counts for the combined-name greps are totals over alternations, not per function.
