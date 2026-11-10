# Unit 46 notes

Reader 46 of 113. Checkout `<checkout>/` (main `457adf4a11f`). All paths below are relative to `omega-rust/psi/compilation/pipeline/`.

Files covered:
1. `04_typed-trees-to-typed-trees/src/preparation.rs` (1-2137)
2. `04_typed-trees-to-typed-trees/src/selection_and_reflection.rs` (1-4260)
3. `05_typed-trees-to-checked-trees/src/borrow.rs` (1-9506)
4. `05_typed-trees-to-checked-trees/src/checking.rs` (1-1408)

Method: every line of all four files was read with the Read tool; counts come from the Grep tool in count/content mode over `omega-rust` (or the stated sub-folder). "Count" below means matching lines including the definition line unless stated.

---

## 1. preparation.rs (04)

**Purpose.** Coordinator for stage 04 ("typed elaboration"). `prepare_typed_trees` (preparation.rs:1941) runs an ordered list of passes over a `TypedTrees` (constants, specialisation of generic calls, operator binding, runtime index fields, alignment queries, mathematical declarations) and freezes it (`program.into()`, 2047). Two smaller entries, `prepare_execution_applications` (2076) and `prepare_constant_execution` (2104), run subsets for build-time evaluation. The header comment (1-6) lists the inline modules.

**Shape (read).**
- `PreparationRequest<'r>` (1825): `admits_pending_inputs`, `deferred_layout_applications`, `asm_authority`, `opaque_property_receipts`, `opaque_representation_selections`, `wire_compatibility_demands`. `settled()` (1836) admits nothing; `preliminary()` (1847) sets `admits_pending_inputs: true`. `PreparationAdmission` (1885) is the owned copy (`admission()` 1895). `TypedPreparation` (1913) carries `mathematics` etc. to checking via `into_checking_judgments` (1922).
- Inline modules: `array_extents::normalize` (8-67); `constant_entries::materialize` (68-325; `has_closed_inputs` 173, `ClosedInputs` 189, `has_open_scope` 253); `default_domain_zero` (334-357); `deferred_constants::require_evaluated_constants` (358-439); `opaque_property_receipts::for_source_instances` (440-501); `runtime_index_fields` (502-1143: `CallerEffects` 519, `contains_reachable_construction` 554, `materialize` 579, `fill` 881, `index_expression` 1050, `capture_recipe` 1098); `runtime_value_applications` (1144-1350); `selections::bind_pre_specialization_authored_selections` (1351-1481); `value_location_alignment::settle` (1482-1813).
- `prepare_typed_trees` order (1941-2056): primitive meanings; reject any machine with `MachineBodyStatus::Deferred` (1947-1953); wire protocol; alignment settle; `require_evaluated_constants` only when `!admits_pending_inputs` (1957); asm discharge if authority given (1960); receipts; `rebound_parameter_homes`; `resolve_dynamic_call_targets`; static argument contexts; open index identities; pre-specialisation selections; a fixpoint `loop` (1985-2018) that stops when `materialized == 0`; then normalise/admit assembly, `settle_value_destinations`, `refine_default_domain_zero`, `elaborate`, `runtime_index_fields::materialize`, dynamic slots, installed-interrupt loop (2029), `array_extents::normalize`, `assignment_destinations::prepare`, domain-membership check, `require_complete_dispatch_subjects`, `check_mathematical_declarations`.

**Inputs/outputs.** Mutates `TypedTrees` in place via `expressions_of_mut`, `push_machine`, `set_fixed_array_length`, statement/type tables. Reads `validation::*` (CheckingContext, CallFrameResolver, canonical places), `crate::monomorphization::*`, `crate::operators`, `crate::constructed_results` (all outside this unit).

**How it decides (read).**
- `array_extents::normalize` (15-66): picks `FixedArray` rows from index `start` whose length is `Expression` of a `ConstExpression` with a valid lifetime owner (35-40). Every failure (no closed integer value, not `Ok(Some)`, not an integer, not `usize`) is a silent `continue` (47-61); length stays unset. Keyed on type-reference handle + owner symbol.
- `constant_entries::materialize` (75-171): an entry machine is added for a const declaration with an entry symbol, no evaluated value and no open scope (78-81). For recipe contexts `live` is: contract expression -> owner among program-scoped proof-fact owners (131-133); `ConstApplication` -> owner among operand type owners (134-136); `_ => false` (138). Entry name `format!("@constant-expression-{}", handle.arena_index())` (161). `ClosedInputs` treats symbol kinds Unknown/Parameter/Local/TypeParameter/MachineParameter/ConformanceParameter/PropositionParameter/PropositionMachineParameter as open, `Const` open iff its scope is open, every other kind closed (208-219).
- `deferred_constants::require_evaluated_constants` (363-438): errors for `ConstCall` lengths with no recipe or `ConstParameter` whose symbol is `Const` (376-402), and for `pending_const_range_endpoints` without contract recipe / deferred contract invocation / const-application root (415-432).
- `opaque_property_receipts::for_source_instances` (443-500): copies a receipt to every data definition whose declaration span equals the receipt's, requiring equal span offsets, file `path`, `package_identity`, `package_root` (only when no identity), `origin`, `resolution_stratum`, `source` (478-486). Source-position keyed. Ambiguous declaration (456) or missing span/file (463, 466) is a `continue`.
- `runtime_index_fields` (579-1096): collects "destinations" (statements/arguments containing a reachable construction) and fills implicit index-descriptor fields of struct literals. Refuses with "cannot validate runtime index descriptor `{}` across argument or field effects without an evaluated scalar snapshot" (1011) and "cannot retain runtime index descriptor `{}` from its application argument without an evaluated scalar subject" (1017).
- `runtime_value_applications` (1162-1349): hoists non-literal (not Integer/Boolean/Float, 1196-1202) const-expression arguments of primitive-typed value parameters into generated locals named `__value_index#{local arena index}#{ordinal}` (1306). Skips inferred, operand-capture, erased, non-`Value` role or initializer-less locals (1245-1251).
- `selections::bind_pre_specialization_authored_selections` (1365-1480): resolves `LateBound` selections of kind CheckedOperator/CheckedMember/CheckedCall; other pairs `_ => None` (1450) and unresolved targets `continue` (1452-1454), staying late-bound. An unknown occurrence is an error (1399).
- `value_location_alignment::settle` (1492-1638): rewrites `align_of(value)` (kind `Alignment`, operand type unset, exactly one argument; 1640-1657). Root must be `Local` or `Parameter` (1727-1732). Slice operand becomes `FirstElement` with a synthesised `[0]` index (1538-1545).

**Cases by shape.**
- `contains_reachable_construction` (554-577): StructLiteral/String/Borrow/Atomic/Cast/Match/ArrayLiteral, `_ => false`.
- `runtime_index_fields::materialize` statement list: LocalData, Expression (only if `state.return_type` valid, 616), Assignment, Call, Transition (Value/Named targets; `_ => {}` 693).
- `runtime_value_applications::operands` (1162-1229): Generic with a unique data definition; recurses through Reference/Constrained/FixedArray/Slice only.
- `align_of` refusals (strings): case-payload field (1743), explicit layout representation (1695, 1761), non-addressable (1735), unknown storage type (1705, 1767), `ensures` (1608), trait requirement contract (1682), element/other projection in a contract (1691), slice in a contract (1712).

**Same job elsewhere.**
- Tree copy with `Name` substitution: `capture_recipe` (1098-1142) and `selection_and_reflection.rs:3420-3462` (`replace_place_alias`) both rewrite `Name` nodes to another expression (read both; different keys: parameter symbol vs alias symbol).
- Generated-local minting via `append_generated_children_from`: preparation.rs:1307-1313 and selection_and_reflection.rs:2889-2897 and :3844-3850 (three sites, read).

**Unused / one-caller (grepped; whole-word counts incl. definition).**
- `prepare_typed_trees`: 23 lines in 8 files (build-time-evaluation `machine_execution.rs` 9, `const_evaluation.rs` 6, `const_initializers.rs` 1; `psi-compiler/source_checking.rs` 1; `05.../immutable_checking_input.rs` 1; 04 `lib.rs` 2; this file 2; one omega example 1).
- `prepare_constant_execution`: 4 (machine_execution 1, immutable_checking_input 1, lib.rs 1, definition 1).
- `prepare_execution_applications`: 7 in 5 files (const_evaluation 1, layouts.rs 2, immutable_checking_input 1, lib.rs 1, this file 2).
- `bind_pre_specialization_authored_selections`: 8 in 4 files (this file 4, lib.rs 1, syntax_probes.rs 1, const_initializers.rs 2).
- `settle_value_destinations`: 3, all in this file (2020, 2058, 2116). `normalize_open_index_identities`: 2, this file only (1971, 2065). `specialize_machine_calls`: 4, this file only (1987, 2030, 2085, 2124).

**Stated limits.** "`align_of(value)` in an `ensures` contract is not yet supported" (1608); "in a trait requirement's contract is not yet supported" (1682); "an element or other projection is not yet supported there" (1691); "needs that policy's placement, which is not yet observed" (1695, 1761); "needs a proof that the case is active, which the query does not yet carry" (1743).

**Fail-open risks.**
- `CallerEffects::inert` returns `true` for every expression when `track_effects` is false (527-529). `track_effects = has_value_parameter_data` (764, 858): false when no data type has value-parameter fields. Then there are no descriptor fields to capture, so effect ordering is not consulted for them. Inferred, not traced to the end.
- `array_extents::normalize` leaves the array length unset on all failure paths (47-61). Whether a later stage rejects an unset `Expression` length was not traced: open.
- `constant_entries::materialize` `_ => false` (138): recipe positions other than the two listed never get a context entry here. Not traced: open.
- `opaque_property_receipts::for_source_instances` `continue`s (454-466) drop copies of a receipt for ambiguous/spanless declarations; the original receipt stays (`instances = receipts.to_vec()`, 447). Traced within the function.
- `selections` `continue` (1452) leaves a selection late-bound; whether later stages reject it was not traced: open.
- `ClosedInputs::extend` `_ => true` (218): symbol kinds not in the open list count as closed. Inferred.

---

## 2. selection_and_reflection.rs (04)

**Purpose.** Three inline modules (header 1-5): `selection` (7-1762) infers type/const/machine/evidence/runtime-value bindings for static-argument call sites; `reflection` (1764-3080) expands the toolchain's compile-time field-visitation and schema queries into generated machines and statements; `result_locals` (3082-4260) refreshes inferred-type locals after specialisation, rewrites dispatch-subject locals and adapts operand captures.

**Shape (read).**
- `selection::callee_proposals` (15-473): `collect_machine_proposals_for_callee` (35), `resolve_callee` (300), `contract_expression_scopes` (328), `enclosing_statement_ordinal` (428). `selection::proposals` (474-514): `StaticProposals {types, constants, machines, evidence, runtime_values}` (482), `BindingContext` (492). `selection::selection_assembly` (515-1018): `selection_for_call` (532), `selection_for_call_with_local_prefix` (590), `selection_from_proposals` (670), `proposal_bindings` (716), `upsert_selection` (925), `unique_complete_selections` (952). `selection::static_bindings` (1019-1365): `infer_static_bindings` (1066), `infer_fixed_array_length_binding` (1248), `infer_domain_argument_bindings` (1300), `same_domain_family` (1333), `same_type_identity` (1344), `same_const_identity` (1352). Top level: `collect_call_proposals` (1395), `collect_contract_call_selections` (1507), `collect_call_selections` (1551).
- `reflection`: `active_case::emit` (1787), `expression_calls::materialize` (2196), `owned_schema::elaborate` (2327), `elaborate_calls` (2518), `is_reflection_declaration` (2506), `emit_fields` (2943), helpers `reborrow_operand` 2816, `reference` 2837, `name` 2851, `borrow` 2865, `local` 2876, `info_field` 2908, `integer` 2932.
- `result_locals`: `dispatch_subjects::materialize` (3110), `completed_subject_type` (3313), `replace_place_aliases` (3346), `normalized_place_aliases` (3375), `place_alias_replacement` (3402), `replace_place_alias` (3420), `require_complete` (3464); `operand_captures::adapt` (3543), `consumed_type` (3693); `reference_observations::materialize` (3789); `InferredUpdate` (3896), `apply_updates` (3903), `refresh_generic_call_results` (3937), `refresh` (4103), `receiver_application_bindings` (4202), `data_application_bindings` (4223).

**Inputs/outputs.** Reads `CheckingInput`/`TypedTrees` tables (type references, expression/statement tables, symbols), `monomorphization::{CallSelection, CalleeState, Candidate}` (outside the unit), `validation::reflection::*`. Produces `CallSelection` rows (selection), rewritten statements and new generated machines/states/symbols (reflection), updated local type references and replaced/removed statements (result_locals).

**How it decides (read).**
- Generic binder match by NAME. `infer_static_bindings` 1082-1087: a type parameter matches if `parameter_symbol == symbol || (both invalid && names equal) || parameter_name == name.as_str()`. The last disjunct matches by spelling alone even when both symbols are valid and different. Same shape for const parameters (1099-1103).
- `same_domain_family` (1333-1342): equal symbols if both valid; else `left.name == right.name` or equal last `::` segment of the names.
- Const literal lookup: searches `named_references()` for a row with invalid symbol whose name equals the literal printed value (88-91, 164-167, 1272-1278). Spelling-keyed.
- `type_reference_is_any_generic_parameter` (891-919): symbol, or name when both symbols invalid.
- `proposal_bindings` conflicts (716-868): type by `normalized_type_identity` (1349); const by `static_const_type_reference_identity` text else type identity (1352-1364); machine by `symbol` only (827); evidence by `symbol` AND `display_name()` (851-852). First binding wins; disagreement sets `conflicted` and a reason string.
- `upsert_selection` (925-950): per `site`, a new selection replaces the old if it binds at least as many slots (`new_evidence >= existing_evidence`, 943).
- `unique_complete_selections` (952-1017): groups complete, closed selections by `SpecializationKey` of normalized identity strings / machine symbols / conformance fingerprints.
- `collect_call_selections` (1551-1761): three sweeps (statement calls not naming declared operators 1566-1601; expressions reachable from signatures/locals/statements/proof output calls 1603-1691; any remaining call in the expression table not covered, not contract, not a source recipe 1711-1758). Skips calls that resolve as named operators (`resolve_named_expression_call`) and statically unreachable contexts (1569, 1661).
- Reflection is keyed on declaration NAME strings inside the toolchain's `reflect.omg`: `is_reflection_declaration` (2506-2516) = `source.origin == Toolchain` and path relative to package root equals `reflect.omg`. `elaborate_calls` then matches `program.symbols.name(declaration)` against "schema" (2537), "inspect_record_fields" (2542), "visit_active_fields"/"visit_active_fields_mut" (2543-2546), "visit_runtime_fields"/"visit_runtime_fields_mut" (2548-2551); anything else `continue` (2552). Data vocabularies are found by `data.name.as_str() == "CaseInfo"` (1820) / `"FieldInfo"` (2663) plus `is_reflection_declaration`. Info fields are found by name strings: `member_identity`, `owner_identity`, `owner_application`, `name`, `declaration_position`, `stable_number`, `has_stable_number`, `qualified_type`, `case_identity`, `is_erased`, `storage`, `query_origin`, `field_info`, `field_loan` (1895, 1939-1966, 2409, 2971-3004).
- Reflection authority (2593-2638): the subject must be selectable from the author's scope, in the author's package and, if cross-package, public (`is_public`); `SchemaQueryAuthority::OwningScope` vs `ForeignScope`.
- `dispatch_subjects::materialize` (3110-3311): for each `DispatchSubject` local, chooses (a) logical alias: statement removed and later `Name` uses replaced by the place when the subject is an anonymous numeric expression (3129-3147) or a proof machine's immutable-denotation place (3178-3197); (b) `DispatchObservation` shared borrow when non-copy storage with builtin coordinates and not mathematical (3198-3213, 3283-3291); (c) plain `Value` (3283). Comment: "A case view observes the same home even when the containing data is copyable" (3161).
- `operand_captures::adapt` (3543-3675): for inferred operand-capture locals used only as `Indexed`/`Range` operands, picks `scalar_operand_read_type` for the selected operator's parameter; falls back to the raw type on disagreement or any use outside operand position (3650-3656). `consumed_type` only attenuates for the eight primitives U8,U16,U32,U64,I8,I16,I32,I64 (3763-3771).
- `refresh` (4103-4197): updates locals whose type is inferred; for calls uses the retained constructed value type and errors if listed as unavailable (4131-4136).

**Cases by shape.**
- Reflection names list above (2537-2552).
- Statement-site visitation: operands must be `[context]` (inspection) or `[subject, context]` (2684-2688); callbacks `[Some(case_callback), Some(callback)]` if active, `[Some(callback)]` otherwise (2579-2587). Sum subject without active callback: "sum visitation requires an explicitly selected active-case callback" (2645-2649). `active_case::emit` requires `variants` non-empty ("requires a sum subject", 1846-1848) and generates `entry` + `case#{n}` states (1849-1856).
- `expression_calls::materialize`: argument count must equal parameter count (2224).
- `elaborate_calls` returns after the FIRST reflection call it expands (`return Ok(true)` at 2540, 2573, 2811) so its caller must loop (caller in monomorphization.rs, not read).
- `borrowed`-style walkers here: `borrowed`... n/a. `receiver_application_bindings` follows only `Member` chains (4210).

**Same job elsewhere.**
- The `native_code_layout` redirect to the first state of `query.operand.symbol` is written twice inside `selection_assembly`: 545-561 and 604-632.
- Generated helper machine construction (`Machine { symbol, name, service_reach_row, service_reach_is_installation_bound, target, authored_target, body_is_present, body_status }`): `active_case::emit` 1863-1873 and `expression_calls::materialize` 2241-2253; the latter also copies `suspends`/`blocks` (2248-2249), the former does not.
- Place alias substitution (3346-3462) vs `preparation.rs:1098-1142` as above.
- Local-minting helper `local` (2876) vs `result_locals::reference_observations` (3843-3850) vs `preparation.rs:1305-1313`.

**Unused / one-caller (grepped).**
- `collect_contract_call_selections|contract_expression_handles_from_scopes|unique_complete_selections|collect_call_selections` combined: 14 lines in 2 files (6 here, 8 in `04.../monomorphization.rs`).
- `fn reference|borrow|local|name|is_reflection_declaration` helper names in 04/src: 5 definitions here, 1 in `assembly_and_operators.rs` (not read; suspected same-named helper, not compared).
- `replace_place_alias*`/`place_alias_replacement`/`normalized_place_aliases`: defined only in this file (3346, 3375, 3402, 3420).

**Stated limits.** No "unsupported/not yet" comments; only diagnostic strings such as "typed field visitation requires its subject owner; delegate an owning operation instead of reflecting a foreign carrier" (2616).

**Fail-open risks.**
- `infer_static_bindings` name-only disjunct (1087, 1103): a required parameter whose spelling equals a candidate binder binds to it even if the symbols differ. Open (not traced).
- `collect_machine_proposals_for_callee`: `if !selected.symbol.is_valid() { continue; }` (215) and `if !matches!(kind, State | MachineParameter) { continue; }` (237) skip an argument without recording anything. The slot stays unbound and is reported through `unresolved_*` flags (`proposal_bindings` 733-740). Inferred.
- `Closed(value)` const with no matching named type-reference row (86-100) advances `const_index` without a binding. Inferred.
- `dispatch_subjects::materialize` `continue` when no subject type (3148) leaves the local as `DispatchSubject`; `require_complete` (3464-3523) then errors "cannot determine the dispatch subject storage type". Traced inside this file.
- `operand_captures::consumed_type` returns `actual` on every unrecognised case (3743, 3758, 3774): conservative.

---

## 3. borrow.rs (05)

**Purpose.** Builds `checked_trees::BorrowFacts`: for every state of every machine, the writable roots, the loans each statement forms, the borrowing calls with their argument accesses, each loan's last use, and the `StateBorrowFact` row (header 1-28). Entry `build_borrow_facts` (9437) runs machines in parallel (`par_iter`, 9452-9471) each on private arenas, then appends them in roster order with handle shifting (`machine_rows`, 7970-8319). Later stages (`checks_borrows`, flow) consume the rows.

**Shape (read; top-level module map).**
- `accesses` (36-1225): `BorrowAccessCollection` (collection mod, 65-286), `completed_dispatch` (287-420: `retain_completed_dispatch_extractions` 307, `transition_invocation` 396), `place` (421-637: `is_borrowed_parameter_root` 434, `BorrowAccessGeometry` 463, `is_image_storage_root` 501, `borrow_access_place` 515, `borrow_access_geometry` 532), `read` (638-769), `records` (770-800), `replay::rejoin_expression_call_accesses` (801-907); `ArgumentAccessSchedule {Invocation, TransitionTarget}` (926), `collect_argument_accesses` (931), `dispatch_target_value_reads` (978), `collect_target_argument_accesses` (1027), `collect_formal_argument_accesses` (1073), `collect_formal_argument_evaluation` (1095), `is_copyable_value_type` (1204), `is_owned_copy_type` (1216).
- `calls` (1226-1893): `BorrowCallCollection` (1238), `collect_expression_borrow_calls` (1332), `reference_custody` (1446-1738: `append_reference_custody` 1463, `replaces_reference_loan` 1659, `retain_result_input_lifetimes` 1686), `collect_transition_target_borrow_calls` (1745), `collect_statement_borrow_calls` (1793).
- `children::recurse_shared_children` (1894-1964).
- `last_uses` (1965-2437): `place_is_used_after_statement` (2255), `update_state_loan_last_uses` (2273), `extend_last_use` (2420), `owner_path_overlaps_place_segments` (2172).
- `lifetime_contract` (2438-3046): `source_type` (2446), `retained_inputs` (2772), `same_path` (2825), `output_input_accesses` (2843), `completed_output_retention` (2955).
- `loans` (3047-7969): `aggregate` (3107-3381, `borrowed_initializers` 3138), `owner_paths` (3382-3654), `returned_carriers` (3655-4648), `replacement` (4653-4733), `stored_calls` (4734-5226), `types` (5229-5269), `descriptor_loads` (5273-5412), `invocation_results` (5415-5671), `reference_borrows` (5673-5916), `aggregate_loans` (5923-6567), `call_views` (6568-7307: `CallViewReturnContract` 6636, `call_view_return_contract` 6770, `rebase_borrow_places_following` 7101, `retained_reference_lineage` 7267); structs `StatementBorrowLoan` (7322), `RebasedBorrowPlace` (7341), `BorrowFormationSources` (7368); functions `retain_backing_contents` (7417), `returned_value_loans` (7524), `statement_borrow_loans` (7658), `binding_borrow_loans` (7699), `value_borrow_loans` (7819), `assignment_borrow_loans` (7858), `is_descriptor_load_observation` (7959).
- `machine_rows` (7970-8319), `owner_geometry` (8320-8459: `named_carrier_path` 8340, `carrier_path` 8373), `roots` (8460-8547), `state::append_state_borrow_facts` (8548-8810; `append_carrier_loans` 8849; `grouped_carrier_loans` 8999; `retire_replaced_carrier_loans` 9081; `append_incoming_reference_trackers` 9118), `supplied_outputs` (9247-9355), `tracker` (9356-9406: `StateLoanTracker` 9376, `BorrowOwnerSegment` re-export 9363), `pub(crate) use validation::borrow_contracts as view_link` (9407).

**Inputs/outputs.** Reads `&dyn validation::CheckingInput` (expression/statement/type tables, `place_at`, `CallFrameResolver`, `validation::borrow_contracts`, `reference_result_custody`), writes arenas (`BorrowFactArenas`, 8580) that become `BorrowFacts::with_roots` (9493). `CheckFacts` (flow) is also read by `completed_dispatch` and `returned_body_source_mismatch` (3908) and mutated by `retain_completed_dispatch_extractions` (307).

**How it decides (read).**
- Places and owner paths are keyed on symbol identity. `owner_segment_matches_place_segment` (3513-3555): `Field` matches if `!place_symbol.is_valid() || owner_symbol == place_symbol || any_field_owner(..).path_identity()` equal (3525-3532), so an invalid place field symbol matches every owner field (overlap-side conservative); `FixedIndex` vs `Index{expression}` compares constant integer value, `is_none_or` so a non-constant index matches (3551). `place_path_covers_owner` (3595-3622) is the retirement-side opposite: `DynamicIndex` covers nothing (3603), invalid field symbol covers nothing (3617). `owner_path_selects_leaf` (3629-3653) is exact-leaf.
- Name spelling is used in some places: `statement_uses_local_name`/`expression_uses_local_name` compare `member.as_str() == local_name` (1995-1998, 2036, 2077, 2138) to decide whether a statement uses a loan owner (2325, `usage` mod). A case selection compares `program.symbols.name(*variant) == selected.as_str()` (4408). `uses_local_name` appears on 21 lines, all in this file (grepped over `pipeline/`).
- Argument accesses (1027-1198): each argument's receiving formal type decides the access kind. Forwarded reference (formal is a reference shell and the value is a reference or a slice view naming an existing place) records the formal's access (1119-1149); an implicit shared loan records `Read` (1150-1173); otherwise `collect_single_expression_argument_access`: authored `Borrow` records its own kind, anything else `collect_read_accesses`. A copyable completed value ends earlier holds (`finish_completed_value` 216-244, `append_read_access` 173-191).
- `recurse_shared_children` (1909-1963) skips `binary.right` when the left is statically `false` for `And` or statically `true` for `Or` (1944-1951): reads inside the skipped operand are not recorded.
- `borrowed_initializers` (3138-3295): reference -> a Reference initializer; `Constrained` -> base; `FixedArray`/`Generic`/`Named` -> aggregate-value initializer (direct `Call/Indexed/Member/Name` or non-recast `Cast`, when the substituted type returns a borrow, 3297-3322) else struct/array literal fields recursively (3324-3380); `ConstExpression|Slice|DynamicTrait|Unit => Vec::new()` (3290-3293).
- Returned-descriptor completion (`completed_field_sources` 4062-4108, `judge_completed_field_sources` 4110-4202): requires the callee `MachineSupplyMode::CheckedBody` with body present (4071) else `None` (unavailable) which falls back to declared candidates (3886-3892). Matches sources to declared fields by owner path, source kind, parameter index and access kind (4170-4182); a source matching no field returns `Err(parameter_index)` (4190-4192).
- `returned_body_source_mismatch` (3908-4001): returns the parameter index of a returned loan not covered by the declared lifetime frontier.
- Lineage: `retained_reference_lineage` (7267-7306): `Reborrow{parent}` only for reference formation with a retained parent; `UnretainedDerived` for casts/calls not declaring a direct view source (7284-7292); else `DirectRoot` when no source owner (7293).
- `call_declares_direct_view_source` (6841): "the promotion gate for call-result loan provenance": true only for `ViewReturnSource::Parameter`.
- `append_carrier_loans` (8849-8991): a derived loan gets `permission_sources` only when every supplier is found in `history` with the same owner and a legal reborrow effect; otherwise cleared (8888-8915). `DirectRoot`/`Reborrow` loans become permission sources unless `ReturnedDescriptor` formation that is not a completed invocation return (8946-8965).
- `last_uses` (2273-2418): fixpoint over trackers extending `last_use_statement_index` through supplier chains and carrier/alias overlap; a mutable-parameter-owned tracker with a non-empty owner path is pinned to the last statement (2348-2362).

**Cases by shape.**
- `recurse_shared_children`: Match, Atomic, ArrayLiteral, Range, Binary, Cast, Unary, StructLiteral; `_ => return false` (1960).
- `collect_expression_borrow_calls` (1332-1444): Call, Indexed, Member, Borrow; `_ => {}`.
- `collect_place_evaluation_reads` (648-676): Name, Member, Indexed, Borrow, Cast, else shared children or read.
- `relative_result_loans` (4299-4522): StructLiteral/ArrayLiteral, Call, Indexed, Member, Borrow; `_ => return Ok(None)` (4436).
- `aggregate_expression_borrow_loans` (6171-6314): Call, Name/Member/Indexed, non-recast Cast; `_ => Vec::new()` (6312).
- `reference_local_borrow_loans` place inference (5792-5844): String, Call, Indexed/Member/Name; `_ => None`.
- `carrier_path` (8373-8458): fixed arrays only with `FixedArrayLength::Literal` (8394); `DynamicIndex => return None` (8454).
- `owner_path_from_place_segments` (3484-3506): `FixedRange`/`OpaqueRange` map to `DynamicIndex`.
- `is_reference_type` / `reference_borrow_access_kind` (5230-5268): every `TypeReferenceNode` variant listed explicitly (no wildcard).
- `grouped_carrier_loans` sort key lists every `PlaceRoot` variant (9004-9021).

**Same job elsewhere.**
- `same_path` (lifetime_contract, 2825) also defined at `checks_borrows.rs:9894`, `checks_borrows_resources.rs:6172`, `flow_retained_borrows.rs:1181` (grepped definitions; bodies not read, suspected duplicates). `owner_path_matches` (3557) also at `checks_borrows.rs:4121` (suspected).
- Argument-access roster rebuilt in three places within this file: `collect_argument_accesses` called from `calls` (1283), `completed_dispatch` (345) and `replay` (877) as the same collector with different schedules (read).
- `stored_calls::loans` (4875) and `stored_calls::from_effects` (4942) and `retire_replaced_carrier_loans` (9081) each test `!effect.may_select && owner == Symbol(local) && place_path_covers_owner(..)` (4905-4915, 4953-4960, 9094-9103; also 1591-1603 and 1670-1677): the same "definite slot replacement" predicate written at least five times.

**Unused / one-caller (grepped).**
- `build_borrow_facts`: 4 lines in 2 files (`published_facts.rs` 2, this file 2). The header says `facts::build_check_facts` calls it first; grep shows its only outside caller file is `published_facts.rs`.
- `view_link::` (alias for `validation::borrow_contracts`, 9407): 24 lines in 5 files (`checks_borrows.rs` 8, `checks.rs` 1, `checks_content.rs` 1, `checks_borrows_resources.rs` 2, this file 12).
- `supplied_outputs::` 4 lines in 3 files (`checks_borrows_resources.rs` 2, `checks_borrows.rs` 1, `checks.rs` 1). `lifetime_contract::` 9 lines in 3 files (`flow_retained_borrows.rs` 6, `flow_expression.rs` 1, `checks_borrows_resources.rs` 2).
- `retain_completed_dispatch_extractions|rejoin_expression_call_accesses|collect_argument_accesses` combined: 18 lines in 4 files (this file 15, `published_facts.rs` 1, `checks_borrows_calls.rs` 1, `execution_terminal_unit_calls.rs` 1).
- `dispatch_target_value_reads`: 2 lines (def 978; one caller `flow_expression.rs:1613`). `is_owned_copy_type`: 3 lines (def 1216; callers `checks_borrows.rs:5059`, `:5396`). `place_is_used_after_statement`: 3 lines (def 2255, re-export 9409, one caller `checks.rs:7797`). `call_declares_direct_view_source`: def 6841, one use 7280, mentioned in comment 3082 ("resource lineage replay") but no other caller found (grep over `omega-rust`, 3 code lines).
- The `loans` header comment (3078-3086) says `borrow_initializer_expressions` is used by the persistent-storage check and `call_declares_direct_view_source` by resource lineage replay; for the latter, grep found no outside caller.

**Stated limits.** "unknown source relationships remain unretained" (3076); `call_declares_direct_view_source`: "carrier-field sources, ambiguous signatures, and view-free calls all stay deliberately unretained" (6838-6839); "The early borrow producer cannot yet know the reaching stored operand graph" (292); `descriptor_loads`: "Preparation grants nothing" (5276); refusal strings with `TemplateDependent` (7401-7403). Statement-bound liveness: "finer intra-statement ordering can extend the same query" (2252).

**Fail-open risks.**
- `stored_calls::roster_at_expression` returns `Ok(Vec::new())` when the stored-reference relation is `TemplateDependent` and the formation is an independent owned formation (4765-4775); `stored_calls::loans` returns `Ok(Vec::new())` for `TemplateDependent` (4891-4893); `binding_borrow_loans` returns `Ok(Vec::new())`/`continue` for `TemplateDependent` (7764, 7801-7803). Effect: a call whose stored-slot relation depends on an unresolved template yields no loans for that call. Whether such a body reaches checking unresolved was not traced: open.
- `append_reference_custody` returns `Ok(None)` when `frames` is `None` or has no schedule (1475-1479). `frames` is `None` if `CallFrameResolver::new` failed (comment 9433-9436: "`None` retains failed preparation"). Effect: no custody rows. Not traced: open.
- `collect_formal_argument_evaluation` with an unknown formal (`receiving_type` defaults to invalid, 1058): neither forwarded nor implicit loan applies, falls to `collect_single_expression_argument_access`, recording a read for a non-`&` argument (1174). A `&mut`-typed formal of an unresolved target would be recorded as read. Not traced: open.
- `collect_expression_borrow_calls` skips expressions with `static_type_context_is_unreachable` (1336-1341) and `recurse_shared_children` skips statically short-circuited right operands (1944-1951): no comment at either explains why unrecorded accesses are safe; inferred that the skipped code does not execute.
- `retain_completed_dispatch_extractions` `continue`s on any mismatch (326-372): leaves the earlier (longer) holds; conservative.
- `retire_replaced_carrier_loans` uses `.ok()` on `stored_reference_fallthrough_effects` (9089-9093): an error silently retires nothing; conservative (loans kept).
- `owner_segment_matches_place_segment` invalid place field symbol matches any owner field (3525): over-approximates overlap; stated in `place_path_covers_owner` (3589-3594: "Retirement requires definite coverage, unlike access conflict detection").
- `update_state_loan_last_uses`/`extend_last_use`: a use at a statement absent from execution order is ignored (2425-2427). Inferred: inadmissible statements.
- `lifetime_contract::source_type` returns `None` on every mismatch (2452-2519): consumers treat it as no retained edge. Whether `None` is rejected or accepted by consumers (`flow_retained_borrows.rs`, not read): open.

---

## 4. checking.rs (05)

**Purpose.** The one entrance (`lower_typed_trees`, checking.rs:793; the header 6) that takes a frozen, prepared program and runs validation, proof checking, fact building, flow and borrow checks and publishes `CheckedTrees`. Also holds whole-request reuse (`RetainedCheck`, 809) and three inline modules: `call_acknowledgements` (27-375), `program_validation` (376-605), `target_families` (606-741).

**Shape (read).**
- `CheckingRequest<'r>` (756): `declaration_authority: Option<&dyn DeclarationSelectionAuthority>`, `external_roots`; `new` (762), `with_declaration_authority` (770), `with_external_roots` (782).
- `lower_typed_trees` (793) -> `lower_typed_trees_reusing` (938) -> `check_program` (1012-1408). `reuse_retained_check` (911), `admit_request` (991), `merge_overflow` (1002), `RetainedCheck` (809; `judges_preparation_input` 849, `judges_preparation_admission` 863, `strict_dependency_bodies` 875, `strict_frozen_dependency_bodies` 884), `CheckingOutcome` (898).
- `program_validation::PreparedTypedProgram` (383), `prepare_typed_program` (399), `entailment_errors_first` (463), `validate_no_bare_boundary_trait_values` (488), `bare_boundary_trait_name` (574).
- `target_families::validate_portable_references` (624), `PortableReferences` (676).
- `call_acknowledgements::validate_call_acknowledgements` (39), `validate_statement` (79), `validate_expression` (230), `validate_call` (318), `envelope_label` (367).

**Inputs/outputs.** In: `FrozenTypedTrees`, `TypedPreparation` (admission flag, deferred layout applications, `CheckedMathematics`, opaque receipts, representation selections), request, prior dependency-body judgments. Out: `CheckedTrees` (`CheckedTrees::with_roots`, 1392) plus retained judgments. `check_program` order (1012-1407): `CheckingContext::new` with external roots / pending layout geometry / representation selections / prepared mathematics (1032-1038); selected attached method bounds; content custody; `build_proof_plan`; `prepare_typed_program`; `build_check_facts`; `judge_contract_entailment`; quotient admission; direct borrow resources; specialisation identities; `check_checked_facts_recording`; parallel `quotient_eligibility`, cleanup guarantees, runtime proof citations (1151-1171); `finalize_execution`; semantic dependencies; accepted template identities; finalized selections; seven parallel classification passes (1229-1259); mathematical declarations, identity formations, indexed theorems (1279-1391); boundary calls, portable references, boundary-body candidates, provider requirements; declaration descriptions, provider instance publication, proof applications.

**How it decides (read).**
- `validate_call` (318-365): matches a source call to a flow-effects row by `(statement_index, call_ordinal)` (329-332), where the ordinal is the count of calls in the traversal of `validate_statement`/`validate_expression`; errors if `acknowledges_suspend != transitive_may_suspend` or `acknowledges_block != transitive_may_block` ("call acknowledgements must match exactly", 347-358); errors for a suspending call not in direct position (360-364). The call target is described by the spelled name `call.target.as_str()` (284) but matching is by position.
- `bare_boundary_trait_name` (574-604): peels `Constrained`, then looks only at the outermost `Named`/`DynamicTrait`/`Generic` head, resolves through `trait_definition_by_symbol(symbol).is_boundary`; `_ => return None` (597). Only used for data fields, state parameters/returns, trait signature parameters/returns (500-566).
- `target_families::declaration` (704-726): skips (does not scan) machines with `MachinePurpose::ProviderDefaults` or a source target context, `Conformance` symbols, and symbols whose source file `dependency_scope == Build` (706-719). Refuses any other declaration that references a machine with `machine_authored_target(machine).is_some()` (`Extend` 684-698), with "unimplemented: portable target-family contract for ..." (667).
- `prepare_typed_program` (399-455): `opaque_properties` is `PendingBuildSelection` when `allow_pending_opaque_copy` (= `admits_pending_inputs`, 1051) else `Required(receipts)` (408-412).
- `RetainedCheck::judges_preparation_input` (849-857): equal admission, equal external roots and full `TypedTrees` equality (`*self.preparation_input == *program`).
- `admit_request` (991-1000): runs `validate_authored_declaration_selections` only if `request.declaration_authority` is `Some` (995-997); always runs `check_external_root_borrows`.
- `lower_typed_trees_reusing` retains only when `retain && !admits_pending_inputs && deferred_layout_applications.is_empty()` for dependency bodies (967) and marks `admits_pending_inputs` for the retained record when either holds (973).

**Cases by shape.**
- `validate_expression` (255-314): lists every `ExpressionNode` variant explicitly (SavedSubject, Match, Atomic, ArrayLiteral, Binary, Cast, Call, Indexed, Member, Borrow, Range, StructLiteral, Unary, TypeExpression/literals); no wildcard.
- `validate_statement` (87-206): Assembly/AssemblyFact skipped; Assignment, Call, Expression, LocalData (with initializer), Transition (guard `When`, named targets, value targets, terminal).

**Same job elsewhere.**
- The same child walk (Match/Atomic/ArrayLiteral/Binary/Cast/Call/Indexed/Member/Borrow/Range/StructLiteral/Unary) is repeated in `validate_expression` (checking.rs:255-314) and `borrow.rs:1909-1963` (`recurse_shared_children`, which omits Call/Indexed/Member/Borrow/Range handling and short-circuit-skips Binary right); read both.
- `statement_uses_*`, `collect_statement_borrow_calls` (borrow.rs:1793) and `validate_statement` (checking.rs:79) each enumerate the same six statement kinds.

**Unused / one-caller (grepped).**
- `lower_typed_trees|lower_typed_trees_reusing`: 28 lines in 10 files (checking.rs 6, lib.rs 3, machine_execution.rs 9, const_evaluation.rs 4, const_initializers.rs 1, source_checking.rs 1, checks.rs 1, products.rs 1, README.md 1, one omega example 1). The header says `lower_typed_trees` is "the one entrance"; `lower_typed_trees_reusing` is also `pub` and is called directly (counted within these).
- `validate_call_acknowledgements`: definition 39 and one call `published_facts.rs:11230`. `validate_portable_references`: definition 624 and one call 1395.
- `CheckingRequest::new()`: 15 call sites outside this file (build-time-evaluation: `machine_execution.rs` 7, `const_initializers.rs` 2, `const_evaluation.rs` 5; omega example 1) plus `source_checking.rs:702`. `with_declaration_authority` has exactly one caller, `source_checking.rs:703`. So 15 of the 16 external construction sites carry no declaration authority.

**Stated limits.** Header: "Preparation has already closed the executable bodies and call identities; this stage never repairs them" (3-4); "Toolchain-owned selections may remain late-bound until build-time evaluation; ordinary authored selections remain strict in both modes" (790-792); "unimplemented: portable target-family contract" diagnostic (667).

**Fail-open risks.**
- `validate_call` returns without checking when no flow-effects row exists for `(statement_index, ordinal)` (329-334), and `validate_call_acknowledgements` `continue`s for a state with no operational summary (47-54). A call whose acknowledgement mismatches its envelope would pass if the plan lacks a row. Whether the plan always has a row for every call was not traced: open. The ordinal alignment depends on this traversal counting calls in the same order as `flow-effects` (no comment says so beyond the header 34-37).
- `admit_request`: with `declaration_authority: None` (the `CheckingRequest::new()` default) `validate_authored_declaration_selections` is skipped (995-997). 15 build-time-evaluation/example call sites use the default (grepped above). Stated: comment 790-792 says toolchain selections may stay late-bound in that mode; the comment does not say why ordinary authored selections are safe without authority. Status: stated for toolchain selections, open for the rest.
- `PendingBuildSelection` (408-412): when `admits_pending_inputs`, required opaque Copy receipts are not enforced in this stage. Stated by name and `preparation.rs:1957`; whether the final strict check re-runs is outside this unit: open.
- `target_families::declaration` returns `false` for ProviderDefaults, target-context machines, Conformance symbols and Build-scope files (706-719), so references inside those are not checked for portable-family misuse. Stated in the module comment (607-616: "Exact member references remain confined to target-scoped bodies").
- `bare_boundary_trait_name` returns `None` for any type head other than Named/DynamicTrait/Generic and for unknown handles (579-584, 597). Stated: "Borrows ... and container members are other positions and stay out of this gate" (482-487).
- `check_program` uses `expect(filled)` (1262-1268), which panics rather than accepts.

---

- **Coverage:** Read in full: `04.../preparation.rs` 1-2137 (2137 lines); `04.../selection_and_reflection.rs` 1-4260 (4260); `05.../borrow.rs` 1-9506 (9506); `05.../checking.rs` 1-1408 (1408). Total read 17,311. Skimmed 0. Not read 0.
- **Five things a maintainer most needs to know about this unit.**
  1. `prepare_typed_trees` (preparation.rs:1941) is a fixpoint of specialisation passes (loop 1985-2018) bracketed by one-shot passes; `settle_value_destinations`, `specialize_machine_calls` and `normalize_open_index_identities` have no callers outside preparation.rs (grepped).
  2. Reflection (`selection_and_reflection.rs:2518-2814`) dispatches on the declaration NAME string inside the toolchain file `reflect.omg` (2506-2552) and on data names `CaseInfo`/`FieldInfo` and field-name strings; it expands only the first matching call per invocation and returns, so the caller must loop.
  3. Static-argument binding matches generic binders partly by spelling (`infer_static_bindings` 1082-1087, 1099-1103; `same_domain_family` 1333-1342; const literals found by printed value 88-91), not only by symbol identity.
  4. `borrow.rs` is a loan ledger built per state with one `StateLoanTracker` roster; the same "definite replacement" test and the same call-argument access collector are reused with different schedules (`ArgumentAccessSchedule`, 926). Many outcomes return empty/`None` for template-dependent or unavailable relations (4765-4775, 4891-4893, 7764, 7801-7803, 1475-1479); none of those sites states why an empty result is safe.
  5. `checking.rs` makes declaration-authority admission optional: only `source_checking.rs:703` supplies it; 15 other `CheckingRequest::new()` sites (build-time evaluation, one example) skip `validate_authored_declaration_selections` (checking.rs:995-997).
- **What you are unsure about.**
  - Whether callers of `lifetime_contract::source_type` (borrow.rs:2446) treat `None` as reject or accept (the consumers in `flow_retained_borrows.rs`/`checks_borrows_resources.rs` were not read).
  - Whether unresolved `TemplateDependent` stored-reference relations can reach borrow-fact building in a checked program (borrow.rs empty-result sites above).
  - Whether `same_path`/`owner_path_matches` duplicates in `checks_borrows.rs`, `checks_borrows_resources.rs` and `flow_retained_borrows.rs` have identical bodies (only definitions grepped).
  - Whether the flow-effects plan always contains a row for every call, which `validate_call` (checking.rs:329-334) silently relies on.
  - Whether `array_extents::normalize` failures (preparation.rs:47-61) are rejected downstream.
  - The `assembly_and_operators.rs` helper with one of the same names as reflection helpers was not read.
