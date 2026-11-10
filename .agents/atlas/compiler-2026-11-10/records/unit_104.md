# Unit 104 notes

Files covered (all under `omega-rust/psi/compilation/validation/src/`, checkout `worker-23`, main `457adf4a11f`):
1. `reference_result_custody.rs` lines 1-9418 (9418 lines)
2. `structural_call_custody.rs` lines 1-3970 (3970 lines)
3. `trace_flags.rs` lines 1-57 (57 lines)
4. `traits.rs` lines 1-8270 (8270 lines)

Total 21,715 lines, all read in full (read in chunks of <=1,800 lines; no skimming).

Method for counts: `grep -rwo --include=*.rs NAME .` over `omega-rust`, minus the same word count inside the four unit files ("outside_unit"). Word counts include unrelated items that share a name, so common names are noisy and marked.

---------------------------------------------------------------------------
## 1. reference_result_custody.rs (9418 lines)

### Purpose
Decides which input storage a returned reference (or a record/array holding references) borrows from, and checks that a caller which receives such a result holds the matching loans. It publishes "origin" plans (`CheckedReturnedReferencePlan`, `CheckedReferenceOriginSourcePlan`) per result leaf, computed from the borrow/flow facts of the callee body, and exposes helpers for the loan groups of locals/storage, release boundaries, and reference multiplicity. Header comment (l.1-4): "Returned reference custody, loan provenance, and region correspondence across call boundaries".

### Shape (sub-modules, with line ranges)
- `stored_carrier_sources` (25-350): `enum StoredCarrierSource {Place, Image, Borrow, Call}` (34); `stored_carrier_sources` (49) -> `select` (67), a recursive walk over `CheckedStructuralValueKind` (Reference, ViewSubslice, BorrowedSliceView/MutableSliceView, NativeCodeLayout, ByteLiteralView, Place/CopiedStructuralPlace, Projection, Dispatch, FixedArray, record/case pattern macro, Call, Case).
- `call_reception` (352-566): `retained_call` (361) finds the unique `FlowCallFact` occurrence; `record_call_reception` (389) returns `RecordCallReception{residual_paths}` = result leaves not covered by loans the receiving local owns.
- `captured_returns` (568-5564), the large engine. Entry points `returned_sources` (4137), `call_sources` (4147), `invocation_borrow_sources` (4207), `terminal_call_sources` (4219), `call_result_paths` (4252). All go through `relations::complete` (3723), a fixed-point loop over per-state `ReturnEquation` rows. Context struct `Captures` (4262: `program`, `facts`, `calls: &mut CompletionCalls`, `relations`, `observed`). Sub-modules:
  - `completion_calls` (570-819): memo tables (`CompletionCalls`, 592) keyed by `(arena_index, generation)`; ambiguous site -> `None`.
  - `completions` (820-1204): `ReturnCoverage` (835), an AND/OR graph (`Coverage::{Call,All,Any}`, 828) built by `Builder` (904) from `FlowOperandFact`s; `established` (891) evaluates it against the set of "productive" states.
  - `descriptor_sources` (1205-1395), `family_returns` (1396-1473: target-family interfaces return through every body's equation), `incoming_origins` (1474-1703), `input_views` (1704-2083), `invocation_inputs` (2084-2414), `invocation_returns` (2415-3035: `invocation_return_sources` 2425, `checked_input_origin` 2757, `captured_input_sources` 2833, `completed_operand_source` 2936, `completed_operand_leaf` 2987), `loaded_origins` (3036-3205), `presence` (3206-3616: `call_paths` 3216, `expression_leaf_present` 3284, `place_leaf_present` 3424, `ingress_present` 3594), `relations` (3617-4116).
  - Inside `impl Captures` (4294-5455): `target_sources` (4311), `returned` (4338), `normal_returns` (4368), `return_origins` (4706), `return_origin_with_frame` (4864), `publish_frame_origin` (5195), `frame_origin_path` (5251). Free fns `frame_origin_fits_loan` (5457), `captured_address_refines` (5487).
- `completed_loads` (5565-6172): `PreparedDescriptorLoadSlots` (5599; roster `SlotRoster::{All,State,UnavailableState}` 5605) and `completed_descriptor_load` (6077), `complete_descriptor_load` (6113), `unchanged_input_slot` (6025).
- `declared_frontier_tests` (6178-6398, `#[cfg(test)]`).
- `declared_returns` (6399-6605): origins from a public signature alone (`returned_sources` 6425, `installed_context_sources` 6413, `invocation_returned_sources` 6562).
- `source_paths` (6606-6818): `reference_origin_path` (6615), `parameter_reference_origin_path` (6729), `structural_path` (6752).
- Top level (6819-9418): limits `MAX_REFERENCE_ORIGIN_LEAVES=4096`, `MAX_REFERENCE_ORIGIN_VISITS=8192`, `MAX_REFERENCE_ORIGIN_DEPTH=128` (6827-6829); `is_mutable_structural_parameter` (6837), `origin_formal` (6853), `origin_index` (6872), `is_reference_record` (6988), `initializer_source` (7046), `borrowed_initializer_source` (7135), `aggregate_projection` (7292), `whole_reference_sources` (7423), `terminal_call_reference_sources` (7556), `record_carrier_sources` (7602), `DeclaredCarrierReference` + `declared_carrier_references` (7638/7645), `declared_input_reference` (7798), `stored_carrier_ingress_is_complete` (7952), `input_carrier_type` (8000), `local_record_loans`/`named_place_loans`/`storage_place_loans`/`record_leaf_loan_groups` (8026-8449), `initial_reference_establishment` (8193), `replaced_reference_carrier` (8240), `loan_place_segments`/`origin_place_segments`/`origin_place_projection` (8451-8636), `loan_owner_path`/`origin_owner_path` (8638-8669), `active_record_loans` (8693), `transient_call_record_loans` (8769), `record_argument` (8883), `classify_program_reference_custody`/`reference_custody` (8963/8967), `parts`/`stored_loan_parts`/`mutable_borrowed_parts`/`shared_borrowed_parts`/`has_borrowed_parts`/`shared_place_borrowed_parts` (8994-9054), multiplicity helpers (9068-9125), `source_parameter`/`source_leaf` (9127/9138), `result_loans` (9196), release helpers `whole_local_release_boundary`, `local_storage_loans`, `storage_loans`, `loan_weakening`, `release_statement`, `release_boundary` (9272-9410), `state_flow` (9412).

### Inputs and outputs
Reads `&dyn CheckingInput` (type-reference table, data members, state parameters, statement table, expression table) and `&CheckFacts` (`facts.flow.control` calls/operands/statements/exits, `facts.borrow` loans/incoming_references/returned_loans/owner_segments, `facts.values.structural_values`, `facts.flow.borrow_lifetimes` activations/weakenings, `facts.flow.ownership`). Produces plans (`CheckedReturnedReferencePlan`, `CheckedUnitStructuralArgumentPlan`, `RecordLeafLoans` = `(CheckedReferenceResultSourcePlan, HandleSpan<BorrowLoanFact>)` at 8021), `ReferenceCustodyFacts` rows (8963), `Option`/`Result<_, &'static str>` answers. Nothing is mutated except local memo tables (`CompletionCalls`, `ReturnRelations`, `EquationObservations`); `relations::complete` is a fresh local per call (3728-3735).

### How it decides (keys)
- Mostly identity keys: arena handles `(arena_index, generation)` (`key` fns 580, 3673; `handle_key` 5837), `SymbolHandle`, `ExpressionHandle` + `statement_index` + `call_ordinal`, `CheckedMemberIdentity::of_member(identity, name)` (ordinal plus name; 20 uses in the file, grep -c). `normalized_type_identity` equality for types.
- Spelling-keyed: `trace_return_state` (4295-4309) compares a machine/state `name` string to the `OMEGA_COMPILER_TRACE_COMPLETION` env list (diagnostic output only). `LocalWriteSource.path` is filled with `symbols.name(root).to_owned()` (e.g. 1294, 1379, 2025, 2816, 2917) as a label, not compared at those sites.
- Position/order: `order.precedes(...)`, `order.successor(...)` in release boundary code (9285-9390); statement index equality for call/loan matching (e.g. 436-445, 4489-4493).
- Structure: `Coverage` AND/OR graph and the productivity fixed point decide whether a recursive return is "independently covered" (`complete`, 3947-3971).

### Cases by shape (fixed lists, refusing the rest)
- `select` (67-349): `_ => return None` (345) for any `CheckedStructuralValueKind` not listed. `ViewSubslice`, `BorrowedSliceView`, `ByteLiteralView` only accept an empty selection (104-106, 143-145, 174-176). `NativeCodeLayout` accepts only `[Field(selected)]` and only the second declared field (`fields.nth(1)`, 152-170). `FixedArray` accepts FixedIndex/ComputedIndex/RuntimeIndex, `_ => return None` (215).
- `record_argument` (8883-8961) and `call_result_field_argument` (8832): `let [destination_leaf] = ... else return None; destination_leaf.path.is_empty()` - exactly one, direct reference leaf.
- `initializer_source` (7046): only a single-name path whose symbol is a parameter or immutable local; `borrowed_initializer_source` (7135): only Member/Indexed projections with an integer-literal index (7231-7246).
- `aggregate_projection` (7292): roots are Name or Call only (7316-7328); index must be an integer literal below a literal array length (7384-7396).
- `source_leaf` (9138): requires one `Parameter` source and a path ending in `Referent` (9156-9165). `source_parameter` (9127): only parameter-sourced, empty-path leaves.
- `declared_carrier_references` (7645): FixedArray only with `FixedArrayLength::Literal`; non-`CheckedShape` data -> `None` (7750-7752).
- `structural_path` (6752): only Case/Field/FixedIndex segments, `_ => return None` (6813).
- `reference_origin_path` (6615): windows (FixedRange/OpaqueRange/Index) handled in a fixed set of shapes, `_ => return None` (6709, 6722).
- `whole_reference_sources` (7423): only Field and FixedIndex segments, `_ => return None` (7516), requires `borrowed` true.
- `family_returns::entries` (1405): only the target-family interface shape, requires first state symbol equality and all bodies equal on return type and each parameter's `is_const/is_mutable/relevance/type` (1413-1437).

### Same job elsewhere
- Three PlaceSegment<->checked-path translators in the unit: `source_paths::structural_path` (reference_result_custody.rs:6752-6817, Case/Field/FixedIndex), `origin_place_projection` (8503-8636, reverse direction), and `structural_storage_path` in structural_call_custody.rs:3256-3334 (Case/Field/FixedIndex again, with erased-field and payload-namespace checks). Each re-implements the Case-then-payload-Field walk.
- Three walkers over the same type shape: `reference_origin_expansion_is_bounded` (6885), `is_reference_record::visit` (6988), `declared_carrier_references::visit` (7645). Each separately handles Reference/Constrained/FixedArray/Slice/Named and data fields.
- `LocalWriteSource { invocation_output: None, invocation_result: None, is_image_storage: false, recasts: Vec::new(), representation: None, path, collection_coarse: false, root, segments }` is written out literally 6 times (1288, 1373, 2019, 2120, 2810, 2911; `grep -n "crate::LocalWriteSource {"` = 6).
- `frame_origin_fits_loan`/`captured_address_refines` (5457/5487) and `paths_overlap` (5522, test-only) each define place-segment overlap relations.
- `is_mutable_structural_parameter`/`origin_formal`/`origin_index` (6837/6853/6872) are the "one predicate" for the structural-argument roster (comment 6831-6836 says published origins and lowering must count the same slots). Outside users (grep, `validation::reference_result_custody::` in `05_typed-trees-to-checked-trees`): `is_mutable_structural_parameter` 1 (execution_terminal_unit_types.rs:1810), `origin_index` 1 (flow_expression.rs:3556), `origin_formal` 3 (checks_borrows_resources.rs:5459, execution_terminal_unit_control_statement_sequence.rs:4166, flow_expression.rs:3398). No hit in the `06_` lowering crate for these three names (suspected: lowering recomputes the roster itself; not read).

### Unused / one-caller (grep, outside_unit counts; counts are word hits and noisy where noted)
- `record_call_reception`: 4 total, 0 outside the unit (only structural_call_custody.rs).
- `transient_call_record_loans`: 2 total, 0 outside. `terminal_call_sources`: 2 total, 0 outside. `call_result_paths`: 3, 0 outside. `completed_descriptor_load_slot`: 2 total (definition + re-export), 0 outside. `borrowed_initializer_source`: 3, 0 outside (called from `initializer_source` in the same file).
- `shared_place_borrowed_parts` (9047): total=1, i.e. the definition only. No call site in `omega-rust`.
- `shared_borrowed_parts`: 4 total, 0 outside (internal only).
- `PreparedDescriptorLoadSlots`: 9 total, 4 outside. `record_argument`: 2 total, 1 outside. `initial_reference_establishment` 3/2. `replaced_reference_carrier` 4/3. `classify_program_reference_custody` 2/1. `stored_carrier_sources` 15/6. `installed_context_sources` 4/1. `call_return_sources` 3/1. `invocation_borrow_sources` 5/1. `stored_carrier_ingress_is_complete` 4/2. `parameter_reference_origin_path` 3/1. `origin_owner_path` 5/1. `is_copyable_stored_value` 3/1. `mutable_borrowed_parts` 5/1.
- Noisy names (shared with unrelated items): `source_parameter` 78/77, `loan_owner_path` 93/77, `result_multiplicity` 51/43, `stored_loan_parts` 53/29, `source_leaf` 25/24.
- `paths_overlap` is `#[cfg(test)]` (5521) and used only by its own test (5549-5556).
- "Single/shared" claim: comment at 6831 (is_mutable_structural_parameter "has to be this one predicate"): 1 outside user in `05_`, none in `06_`/`omega/` by name (see above).

### Stated limits
- "Signature lifetime dependencies remain in the borrow facts. They describe retained inputs, not an executable descriptor" (8438-8440): such loans are skipped (`continue`) in `record_leaf_loan_groups`.
- `descriptor_alternatives` / many `Err("...unavailable...")` strings are the refusal vocabulary (e.g. 3105, 3117, 3139, 3144).
- `relations::complete` refusals: "captured return alternatives are not monotone" (3757), "recursive source is outside its captured frontier" (3911), "recursive return has no independently covered value boundary" (3953).
- Bounds: leaves 4096, depth 128, visits 8192 (6827-6829); exceeding returns `None` (e.g. 1354, 1778, 2396, 4474).

### Fail-open risks
- `returned` (4343-4345): `if active.contains(&state.symbol) { return Some(Vec::new()) }` - a recursive occurrence contributes no origins. "traced": the only entry through `returned` with a non-empty `active` is within the fixed-point loop, where each equation is re-evaluated with `relations.approximation` (4311-4335) and later iterations check monotonicity (3755-3760) and the recursive frontier (3860-3915). So a wrong cycle cannot be accepted by this empty answer alone; but `target_sources` (4311) outside `complete` returns `None` when `relations` is `None` (4335).
- `expression_leaf_present` -> `Some(false)` for `StoredCarrierSource::Image` (3354-3356), then `normal_returns` accepts a leaf with no loan when `!present` (4516-4535, only `if present {return None}`). Input reaching it: a returned reference leaf that is an "image" value (byte literal view, native code layout, subslice of one). Which wrong program: none identified for ordinary programs; "open" (no comment at site; I did not trace that every Image source is truly storage-free).
- `record_call_reception` early `Ok(anonymous())` returns (425, 457, 464-465, 476-478, 513-514): every returned path becomes "residual" (uncovered). "traced": the only caller (`structural_call_custody` 3598-3611) then demands `transient_call_record_loans` for residual paths and errors otherwise, so the default is closed.
- `transient_call_record_loans` (8795-8799): `CheckedReferenceOriginRoot::Invocation { .. } => return true` inside `.all(..)` - sources rooted at an invocation are accepted without comparing any argument. Input: a record result whose residual leaf has an invocation-rooted (independent) origin. "open": no comment; I did not trace whether the callee's own check covers it.
- `is_reference_record` (6988-7044): `visit` returns `None` for data whose `supply_mode != CheckedShape` (7020-7022) and the final `== Some(true)` makes that `false`. A boundary/opaque data value that stores a reference would then not be a "reference record" (so `local_owes_record_loans` 8012 is false). "open": `declared_carrier_references` returns `None` for the same type (7750-7752) so record-carrier queries refuse, but this gate is evaluated first by several callers (not all read).
- `reference_origin_expansion_is_bounded::visit` (6958): `_ => return Some(())` for unlisted type nodes counts zero leaves (stated: no).
- `declared_carrier_references` returns `Some(())` with no leaf for `NumericOwned` contents (7685-7687) and `activation_claim_byte_carrier` names (7742-7747) - treated as holding no references. "open".
- `ReturnCoverage::established` uses `values.get(self.root).copied().unwrap_or(false)` (900): fails closed.
- `unique_state_at` (5857) returns `None` on duplicate state rows: fails closed.

---------------------------------------------------------------------------
## 2. structural_call_custody.rs (3970 lines)

### Purpose
Builds the checked plan for a call whose arguments/results are structural values: which linear claims cross at owned arguments, which claims the result carries, which reference loans/qualifications the result has, plus the "declaration predicate" rows (a data type's `where` facts turned into checked boolean rows on a state's parameters) and the structural qualification inventory (domains attached at each path of a type). Header (1-4).

### Shape
- `declaration_predicates` (25-2254): `DeclarationPredicateRow{parameter_position, restored_on_return, predicate}` (269); `state_declaration_predicates` (275), `machine_declaration_predicates` (349), `declaration_facts_carried` (361), `complete_definition_storage` (375), `definition_emits_rows` (784), `RowEmitter` (500), `FactSpelling` (904: `boolean` 924, `comparison` 1089, `scalar` 1194, `field_path` 1306), `field_carriers` sub-module (47-251: `imply_declaration` 58, `FieldCarriers::boolean/integer` evaluating a predicate by the carrier's integer bounds), `carrier_bounds` (770, only I8..I64/U8..U64), tests (1468-2253).
- `state_selection_tests` (2255-2346, test).
- `StateContractRow` (2354), `restates_structural_parameter_qualification` (2366), `structural_state_contract_predicates` (2398), `finite_arrival_parameter` (2493), `state_finite_predicate` (2543), `scalar_declared_domain` (2575).
- Qualification: `StructuralResultContract` (2598), `declared_carrier_qualifications` (2603), `structural_result_contract` (2649), `structural_result_qualifications` (2666), `view_result_carrier_qualifications` (2673), `result_membership_carrier` (2687), `structural_qualification_inventory` (2748, inner `collect` 2754), wrappers 2716-2746.
- Claims: `reconstruct_structural_parameter_return_claims` (3057), `returned_claim_outcomes` (3172), `retained_claim_segments` (3201), `structural_claim_path` (3212), `structural_owned_path` (3226), `structural_transfer_storage_path` (3236), `structural_storage_path` (3256), `borrowed_slice_view_result` (3336).
- Call plan: `structural_authored_arguments` (3349), `semantic_caller_state` (3419), `structural_call_custody` (3439) -> `CheckedStructuralCallCustodyPlan{reference_loans, retained_reference_inputs, forwarded_reference_sources, result_qualifications, claim_transfers, result_claims}`, `structural_call_result_claims` (3681), `possible_returned_claim_paths` (3757), `owned_argument_claim_transfers` (3786), `received_result_claims` (3910).

### Inputs/outputs
Reads `CheckingInput` (types, data members, state params/contracts, proof facts), `CheckFacts` (`flow.control` states/calls, `flow.ownership` permissions/claim_outcome_maps/segments), `CheckedOperatorFacts`. Returns checked boolean expressions, qualification contracts, claim plans, or `Err(&'static str)`. No mutation.

### How it decides
- Identity: symbol handles, `CheckedMemberIdentity`, `PermissionClaimIdentity`, `PermissionEventSource::Call{statement_index, call_ordinal, target_symbol}` (3573), place roots and segment prefix (`starts_with`, 3850, 3863).
- Spelling: `find_data_field` falls back to `field.name.as_str() == name` when the symbol is invalid (897-899); `field_path` falls back to member name text for payload fields when `symbol` invalid (1359-1363). `FieldCarriers::integer` matches fields by `identity.ordinal` AND `identity.name` (188-189, 208-209).
- Value: interval/bounds comparison in `FieldCarriers::boolean` (141-163).

### Cases by shape
- `carrier_bounds` (770-782): `PrimitiveType::{I8,I16,I32,I64,U8,U16,U32,U64}` only, `_ => return None`.
- `FactSpelling::boolean` (924-1023): only Boolean, `!`, `==`/`CaseMembership` with exact case-membership meaning, `&&`, `||`, `<,<=,>,>=,==,!=` over scalar operands, or a bool-typed field path; `_ => None` (1011).
- `comparison` (1089): float comparisons only when `has_builtin_or_sealed_float_comparison_meaning`; integers only when `carrier_bounds` knows the type.
- `parameter_data_definition` (324): only an argument-free `Generic` base (340-342).
- `structural_storage_path` (3256): Field/Case/FixedIndex only, `_ => Err("structural claim has an unsupported or mistyped path")` (3326).
- `structural_qualification_inventory::collect` (2754-3023): array length must be `Literal` or a `ConstCall` with a constant recipe; otherwise `Err("projected qualification array length is not literal")` (2898).
- `reconstruct_structural_parameter_return_claims` (3057): requires `CheckedBody`, non-mutable/non-const source parameter, `Multiplicity::Linear` return, identical type identity of source and return (3085-3094).

### Same job elsewhere
- Path translation duplicates reference_result_custody.rs:6752 and 8503 (see file 1).
- `structural_result_contract`/`declared_carrier_qualifications` (2603-2664) and the `Constrained`/`Domain` peeling in `scalar_declared_domain` (2575) and `finite_arrival_parameter` (2516-2539) each walk the `Constrained` chain separately.
- `own_fact_reuse::original_definition_storage` (1785, test) is kept as an oracle for `complete_definition_storage` (375) (test at 1606-1783 compares them).

### Unused / one-caller (grep, outside_unit)
- `declaration_facts_carried`: 7 total. `grep -rn -w declaration_facts_carried` in `.rs/.md/.txt` shows only: definition (361), re-export at 2349 and lib.rs:1518, and test uses (1600, 2021, 2115, 2152). No production caller anywhere in `omega-rust`.
- `state_declaration_predicates`: 3 outside (execution_terminal_unit_state_graph.rs:9756, published.rs:9910, lib.rs re-export). `machine_declaration_predicates` 2 outside.
- `restates_structural_parameter_qualification` 3 outside; `structural_state_contract_predicates` 2; `finite_arrival_parameter` 1; `state_finite_predicate` 2; `reconstruct_structural_parameter_return_claims` 2; `structural_call_custody` 6; `structural_call_result_claims` 2; `structural_transfer_storage_path` 2; `result_membership_carrier` 6; `structural_qualification_contract` 4; `zeroed_byte_projected_qualifications` 4.
- `structural_result_contract` 12/10 (name also used elsewhere; noisy).

### Stated limits
- "projected qualification array length is a constant recipe fixed at target closure" (2920).
- `structural result custody cannot manufacture an owned reference` (2658).
- Comments in tests: "case validity belongs to the element carrier, not 32 copied predicate trees" (2244).

### Fail-open risks
- `RowEmitter::requirement_rows` (642-656): a proof fact that `FactSpelling::boolean` cannot spell and that has no `data_fact_element_consequences` is skipped (`continue`), i.e. no row is published for it. The fence against that is `declaration_facts_carried` / `definition_own_facts_carried` (471-498), which demands every fact be spellable or an exact consequence. Because `declaration_facts_carried` has no production caller (above), nothing in `omega-rust` enforces the fence. Wrong program: a data type with a `where` fact the spelling cannot express would get predicate rows silently missing for that fact. "open" (I did not read the two callers of `state_declaration_predicates` in `05_`; they might pre-check by other means).
- `complete_owned_storage` (438): `owned_data_definition(..).is_none_or(...)` returns `true` for any carrier that is not a Named/Generic with a closed data definition. "open".
- `state_declaration_predicates` (286-303): `const`/erased parameters and `WriteOnly` references are skipped (`continue`); stated by position only (no comment).
- `structural_qualification_inventory::collect`: `Named|Generic` with no `closed_data_definition` returns `Ok(())` (2953-2955), `_ => Ok(())` (3017), fields whose type is primitive or a byte-sequence carrier are skipped (2980-2987), cyclic types stop silently (`active.contains` 2956-2958). Wrong program: an unresolved named type contributes no qualifications. "open".
- `possible_returned_claim_paths` returns `Ok(None)` for a callee with no state, non-`CheckedBody` supply, or no outcome map (3762-3773); `None` means "no restriction", so every frontier claim is treated as returned (`absent` false at 3714-3716). "traced": `received_result_claims` must supply a matching establishment for each claim or `structural_call_result_claims` errors (3723-3725), so an absent claim without a recorded absent-establishment is refused.
- `structural_call_custody` (3613-3621): `terminal_call_reference_sources(...).unwrap_or_default()` turns a refusal into an empty list; the next branch (3622-3639) requires `result_loans` for borrowed results and errors if none. "traced".
- `owned_argument_claim_transfers`: `continue` on non-owned argument (3828-3830) and on unresolved place (3831-3838), but final check `accounted.iter().any(!accounted)` -> Err (3895-3897): closed.

---------------------------------------------------------------------------
## 3. trace_flags.rs (57 lines)

### Purpose
One process-wide read of each diagnostic environment switch via `OnceLock`, so hot per-operation loops do not take the Windows environment lock (comment 1-11: an unconditional read cost about 0.75 CPU seconds of a one-line `omega --check`).

### Shape
`switch_is_set(cell, name)` (15) plus eight `pub fn`s: `check_sites` (19, `OMEGA_COMPILER_CHECK_SITES`), `trace_completion` (24), `trace_installed_context` (29), `struct_trace` (34, `OMEGA_STRUCT_TRACE`), `call_write_trace` (39), `reference_slot_trace` (44), `storage_query_trace` (49), `state_reference_diagnostics` (54). Each is a function-local `static OnceLock<bool>`.

### Inputs/outputs, decisions
Reads `std::env::var_os(name).is_some()` once; returns bool. Keyed on presence of an environment variable name. String-valued filters keep matching elsewhere (comment 9-10).

### Same job elsewhere
Direct env reads of the same switch names bypass these cached functions: `grep -rn --include=*.rs 'std::env::var(_os)?("OMEGA_<one of the 8 names>"'` outside this file = 38 lines (examples: 05_ flow_call_phases.rs:1697, unit_composed_control.rs:8861 (06_), typed-trees evidence_and_inspection.rs:501/574, contract_entailment.rs:56019/56369, terminal-interpreter engine_terminators.rs:787, omega `02_` control_flow.rs:1237/1268, control_flow_calls.rs:359, coordination.rs:618, references.rs:1422/1581/1620/1625/1709, scalar.rs:1051).
Qualified uses `trace_flags::NAME` (grep): check_sites 66, trace_completion 8, trace_installed_context 3, struct_trace 64, call_write_trace 8, reference_slot_trace 4, storage_query_trace 2, state_reference_diagnostics 4.

### Stated limits / fail-open
Comment: the switches "only add `eprintln!` output and nothing sets them mid-process" (7-9). Fail-open: none seen.

---------------------------------------------------------------------------
## 4. traits.rs (8270 lines)

### Purpose
Validation and derived facts for traits, named implementations ("conformances"), machine-satisfies-trait edges, top-level boundary requirements, dynamic (`dyn Trait`) selection/slot demands, provider-application/region correspondence, refinement fit, and closed conformance applications with canonical identities. Header (1-5).

### Shape (top-level modules, line ranges)
- `conformance` (9-3852):
  - `disallowed_reached_service_names` (10); `bounds` (40-162: `validate_generic_conformance_bounds` 61, `validate_trait_conformance_bounds` 84, `validate_conformance_bounds` 107); `external_shapes` (163-462: `validate_external_leaf_native_shapes` 176, `validate_external_via_expression` 248, `private_native_carrier` 381); `generic_operator` (463-524: `generic_bound_operator_requirement` 472); `machine_conformance` (525-1795: `closed_requirement::resolve` 537, `validate_machine_trait_conformances` 731, `validate_machine_top_level_requirement_conformance` 863, `top_level_lifetime_frame` 1161, `revalidate_top_level_requirement_realization` 1201, `revalidate_machine_trait_conformances` 1230, `validate_top_level_requirement_effect_ceiling` 1251, `validate_machine_operator_conformance` 1326, `validate_machine_single_requirement` 1437, `validate_concrete_named_evidence_contract_conformance` 1564, `validate_named_contract_lane` 1651, `compose_forwarded_trait_arguments` 1768); `signature_matching` (1796-3238: `borrow_refinement::with_contracts` 1805, `evidence::validate` 1872, `region_application::RegionApplicationScope` 1964, `validate_applied_trait_signature` 2183, `validate_trait_effect_ceiling` 2544, `TraitTypeBinding` 2671, `type_references_match_for_trait_bindings` 2699, `fixed_array_lengths_match` 2954, `binding_matches_actual` 3015, `indexed_law_callable_telescope_groups` 3152); `trait_applications` (3239-3728: `validate_trait_application_obligations` 3250, `validate_machine_declaration_identity_arguments` 3378, `validate_proposition_family_arguments` 3449, `indexed_carrier_parameter_matches` 3584, `validate_implementation_type_arguments` 3669); kind helpers `type_parameter_kinds_identical/callable/bindable` (3772-3800); `require_operational_ceiling` (3817).
- `conformance_applications` (3853-5195): `forwarding` (3865: `validate_forwarded_argument` 3877, `applied_bound_argument_identities*` 3923-3982), `validate_data_evidence_application` (4169), `validate_conformance_applications` (4223), `close_conformance_application` (4354), `close_selected_conformance` (4379), `close_const_argument` (4640), `substituted_type_identity` (4903), `application_identity` (5025), `normalize_machine_operand` (5154).
- `dynamic` (5196-6341): `row_descriptions::describe_dynamic_conformances` (5202), `collect_dynamic_implementation_selections` (5453), `DynamicSlotDemand` + `collect_dynamic_slot_demands` (5617/5627), `dynamic_call_demand` (5764), `dynamic_requirement_ineligibility` (5799), `qualified_trait_scope` (5877), `resolve_selected_dynamic_call_targets` (5902), `resolved_dynamic_requirement_symbol` (6145), `closed_family_requirements` (6294).
- `named_implementations` (6342-6663): `validate_member_formations` (6349), `validate_conformances` (6394), `revalidate_named_implementation` (6427), `validate_closed_rows` (6536).
- `provider_applications` (6664-7021): `provider_application_associations` (6686, memoized in `frozen_program_memos`), `closed_requirement_provider_associations` (6696), `derive_provider_application_associations` (6841).
- `provider_regions` (7022-7326): `provider_region_correspondence` (7043), `closed_provider_region_correspondence` (7061), `ApplicationRegions` (7223).
- `refinement_fit` (7331-7543): `resolve_bound_carrier` (7345), `refinement_fit_diagnostics` (7381).
- `requirements` (7544-7733): `validate_trait_requirements` (7553), operator-token duplicate check (7656), cycle detection (7681).
- `trait_implementation_arguments` (7734-8228): `validate_trait_implementation_arguments` (7749), `validate_implementation_application` (7895), `validate_binding` (7964), `validate_retained_implementation_arguments` (8094).
- Re-exports 8229-8259; `trait_definition_by_symbol` (8261).

### Inputs/outputs
Reads `CheckingInput` (traits, conformances, machines, machine_specializations, closed requirement applications, type reference table, proof facts, service-reach rows) and `flow_effects::ServiceReachInferencePlan`. Mostly pushes `Diagnostic::error(String)` into `&mut Vec<Diagnostic>` and `IntegerRangeObligation`s. `resolve_selected_dynamic_call_targets` (5902) is the one mutating entry: it rewrites `target_symbol` on call expressions/statements of `&mut TypedTrees` (6002-6019). `describe_dynamic_conformances` builds `DynamicConformanceFacts` (5316).

### How it decides (keys)
- Identity: symbol handles, `normalized_type_identity`, `package_qualified_type_identity`, lifetime parameter identities, `NormalizedTypeIdentity`, `CallableIdentityId`, canonical conformance identity bytes (5025-5152).
- Source spelling / names: `producer_entry_state` (349-364) selects the producer's state by comparing the machine name's last `::` segment to each state `name`, falling back to the first state; `family_realization_state` (5356-5369) picks the realization state of a specialization by `program.symbols.name(state.symbol) == name`, falling back to `.first()` (5366-5368); `retains_application_tuple` (620-657) finds a const argument's type reference with `named_references().find(|(_, symbol, name)| !symbol.is_valid() && *name == value)` where `value` is the decimal text (641-644); `validate_conformances` duplicate check compares `alias` names (6401-6407); `type_references_match_for_trait_bindings` matches `binding.parameter_name == parameter.name.as_str()` along with symbol (2723-2726, 2967-2969); `type_parameter_binding...`; `requirement_lifetime_matches` compares lifetime names (2936-2951). `bound_type_reference_label` (2631-2668) does text replacement of parameter names inside rendered labels (diagnostic text only).
- Refinement: `refinement_fit_diagnostics` (7381) reports "removes `suspends`/`blocks`" only if the refinement clause has non-empty `suspends_keyword_source_spans` / `blocks_keyword_source_spans` (7490-7505), i.e. keyed on whether the keyword was written in source.
- Position: parameter positions, `state_position` ordinals (6637, 2078).

### Cases by shape
- `type_parameter_kinds_*` (3761-3809): families Type/Implementation/Const/Value/Machine/Proposition; `callable` excludes Proposition, `bindable` allows only Type/Const/Value.
- `validate_concrete_named_evidence_contract_conformance` (1564): named-witness lanes only for fully concrete non-generic trait/requirement/satisfier ("named-witness conformance currently requires a concrete non-generic trait, requirement, and satisfier", message at 1608). `validate_named_contract_lane` handles exactly one `ProofFact::Proposition` per row with no binder arguments (1718-1736, "this first conformance rung is concrete only").
- `validate_external_via_expression` (248-347): exactly one receiverless zero-argument machine call, one producer machine, body-present non-generic zero-parameter entry ("the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state", 341).
- `close_const_argument` (4640): integer const literal, closed const expression, canonical atom, caller binder, or named const; others -> category error.
- `describe_dynamic_conformances` (5202): row source variants `Inline|Synthesized`, `Reference`, `TraitDefault`; family rows `Exact` or `Finite(tuples)` (5319).
- `collect_selections_in` (5509): only `Cast` to `DynamicTrait` whose operand is a name resolving to a `Conformance` symbol; anything else is an error (5555-5560).
- `private_native_carrier` (381): Slice and `core` Vector (and records containing them) only; `ConstExpression|DynamicTrait|Unit => None`.
- `resolve_closed_requirement_application` (537): requires `CheckedBody` provider with body, exactly one specialization receipt, exactly one operator realization application, and a unique matching closed requirement machine; any other count -> `failure()`.

### Same job elsewhere
- `validate_machine_top_level_requirement_conformance` (863-1159) and `validate_applied_trait_signature` (2183-2542) both compare provider/requirement lifetime counts, type-parameter kinds, `TraitTypeBinding` vectors (built twice: 962-972 and 2335-2363), parameter shapes/types, return types, borrow/lifetime frames, contract refinement and effect ceilings. Both effect-ceiling checks share `require_operational_ceiling` (3817) via `validate_top_level_requirement_effect_ceiling` (1251) and `validate_trait_effect_ceiling` (2544).
- `validate_trait_implementation_arguments` (7749) re-walks parents/refinements/bounds/implementations/machine edges, repeating the per-site checks done in `validate_trait_application_obligations` (3250) and `validate_conformance_bounds` (107) via the shared `validate_trait_implementation_application` (7872).
- `refinement_fit_diagnostics` (7381) and `validate_named_contract_lane` (1651) both prove public contract strengthening against a realization.
- `describe_dynamic_conformances` (5202) and `collect_dynamic_slot_demands` (5627) both re-enumerate `collect_dynamic_implementation_selections` (5453) results (two full traversals of every expression root per question).

### Unused / one-caller (grep, outside_unit)
- Zero uses outside the unit: `type_argument_identity` (3 total), `substituted_type_identity` (8), `type_parameter_kinds_identical/bindable/callable` (2 each = def + use), `trait_type_bindings` (5), `dynamic_receiver_call_error` (2), `close_conformance_declaration` (1 total, i.e. definition only; no call site found).
- One or two outside: `resolve_closed_requirement_application` 1, `provider_region_correspondence` 1, `closed_provider_region_correspondence` 1, `closed_requirement_provider_associations` 1, `applied_scope_argument_identities` 1, `dynamic_requirement_ineligibility` 1, `validate_member_formations` 1, `revalidate_machine_trait_conformances` 2, `compose_forwarded_trait_arguments` 2, `collect_dynamic_slot_demands` 2, `dynamic_call_demand` 2, `revalidate_named_implementation` 2, `validate_conformance_applications` 2, `validate_conformance_lifetime_arguments` 2, `resolve_bound_carrier` 2, `refinement_fit_diagnostics` 2, `validate_implementation_application` 2, `validate_trait_application_obligations` 2, `report_dynamic_receiver_call_error` 2, `named_conformance_argument_matches` 2.
- Larger: `close_conformance_application` 35 (name also widely used), `collect_dynamic_implementation_selections` 9, `qualified_trait_scope` 9, `GenericBoundRequirement` 14.
- "Shared" claim: `validate_forwarded_argument` / `resolve_bound_carrier` are the common gates for implementation-slot forwarding (used at 2 sites inside the unit each: 1940, 7998 / 3902, 3948, 7982). Outside users of `resolve_bound_carrier`: 2 (grep).

### Stated limits
- "named-witness conformance currently requires a concrete non-generic trait, requirement, and satisfier" (1608, in diagnostic text).
- "this first conformance rung is concrete only" (1729, diagnostic text).
- "the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state" (341, diagnostic text).
- "boundary-machine requirements are not local dynamic calls", "requirement-local generic parameters outside one finite family" (5807-5811).
- "has no prepared callable telescope for its exact application" (2249).

### Fail-open risks
- `validate_machine_single_requirement` (1517-1531): `let Some(lifetimes) = lifetimes else { return; }` and `AppliedTraitScope::new(..) else { return; }` skip `validate_applied_trait_signature`, named-evidence, and law checks with no diagnostic. Input: lifetime ordinal out of range (caller already errors at 792-802, "traced") or `AppliedTraitScope::new` returning `None` (not traced, "open").
- `validate_named_contract_lane` (1718-1724): rows whose facts are not exactly one `Proposition` on each side are skipped with `continue`, so such rows are never compared. Wrong program: named evidence rows backed by expression/membership facts get no equality check. "open".
- `validate_implementation_application` (7907-7909): `if parameters.len() != arguments.len() { return; }` and `validate_implementation_type_arguments` (3681-3697) return silently on shape mismatch (arity is checked at other sites, e.g. 3262-3269 and 136-144). "traced" for trait applications; "open" for data/conformance paths.
- `type_references_match_for_trait_bindings` (2706-2708): both handles invalid -> `true`. A required trait type parameter not yet bound is bound to whatever actual is and returns `true` (2730-2736) (this is unification, first use binds). "stated": no comment.
- `requirement_lifetime_matches` (2933-2941): `required` is `None` -> true; a required lifetime not among the trait's target lifetimes -> true. Provider side `required.is_none_or(..)` (2912). Wrong program: an implementation whose reference lifetime is unconstrained by the requirement gets no region check. "open".
- `refinement_fit_diagnostics` (7397-7399, 7490-7505): rows with no covering clause are not checked; `suspends`/`blocks` removal is only caught when the keyword source spans are non-empty (source-spelling dependent). Wrong program: a refinement whose clause signature loses `suspends` without a recorded keyword span would not reject a suspending realization. "open".
- `family_realization_state` (5366-5368) and `producer_entry_state` (362-363): fall back to the first state on a name miss. Input: an instance machine whose state names differ from the template state's. "open".
- `fixed_array_lengths_match` (2965, 2971): with no binding, falls back to plain `actual == required`.
- `closed_requirement::resolve` (541-546): if requirement has no type parameters or provider has some, it returns the requirement unchanged (no closure check). Stated by structure (non-generic requirement needs no closure).
- `collect_selections_in` (5563-5565): `let Some(conformance) = ... else { return; }` silently drops a cast whose symbol has no conformance row; the name check at 5549-5551 already requires `SymbolKind::Conformance`, so reachable only on a missing declaration. "traced" (kind check precedes).
- `resolve_selected_dynamic_call_targets`: targets with `dynamic_target == None` fall back to `statement_call_target` (5953-5957) or leave the target unchanged (5958 `if let`).

---------------------------------------------------------------------------
## Coverage
- Read in full: `reference_result_custody.rs` 1-9418 (9418 lines); `structural_call_custody.rs` 1-3970 (3970 lines); `trace_flags.rs` 1-57 (57 lines); `traits.rs` 1-8270 (8270 lines). Total 21,715.
- Skimmed: none. Not read: none.
- Approx. share that is `#[cfg(test)]`: reference_result_custody.rs tests at 686-818, 1030-1203, 5521-5563, 5877-5984, 6177-6398; structural_call_custody.rs 1468-2346; traits.rs 4054-4163.

## Five things a maintainer most needs to know about this unit
1. `captured_returns` (reference_result_custody.rs:568-5564, about 5,000 lines) is one engine: every result-origin question runs through `relations::complete` (3723), a fixed-point over per-state return equations with an AND/OR "coverage" graph for whether a recursive return is independently covered. Recursion in `returned` is answered with an empty list (4343) and is checked afterwards by monotonicity/frontier tests (3755, 3910).
2. Most of the file's refusals are `None`/`Err(&'static str)`; the main "accepting" defaults are `Image` sources counted as not present (3354), invocation-rooted sources accepted in `transient_call_record_loans` (8797), and non-`CheckedShape` data not being a reference record (7020).
3. `declaration_facts_carried` (structural_call_custody.rs:361) is the documented fence for `state_declaration_predicates` silently skipping unspellable `where` facts (644-656), but it has no non-test caller in `omega-rust` (grep -w, 7 hits: def, 2 re-exports, 4 test uses).
4. `traits.rs` splits conformance checking across two parallel implementations (top-level requirement, 863-1159, and trait requirement, 2183-2542) with similar but separately written binding/lifetime/ceiling logic; several silent `return`s skip checks (1517-1531, 7907, 3681-3697) and name-text fallbacks (5366, 362, 641-644) key on source spelling.
5. Several `pub` items have no external caller: `shared_place_borrowed_parts`, `close_conformance_declaration`, `record_call_reception`/`transient_call_record_loans`/`terminal_call_sources` (unit-internal only), `type_argument_identity`/`substituted_type_identity`, and the 6-way `LocalWriteSource` literal and three path translators are repeated rather than shared.

## What I am unsure about
- I did not read callers in `05_typed-trees-to-checked-trees` (e.g. execution_terminal_unit_state_graph.rs:9756, published.rs:9910) to see whether they pre-check unspellable `where` facts; the "open" status of the dropped-fact risk depends on that.
- "Outside_unit" counts are word matches across `omega-rust` and can include unrelated same-named items (noisy names are marked); `pub use` re-exports in `lib.rs` count as outside hits.
- I did not trace whether every `StoredCarrierSource::Image` is truly storage-free (3354-3356) nor the callers of `is_reference_record` for opaque data.
- I did not read `06_` lowering to confirm whether it shares the structural-argument roster predicate (comment 6831-6836); only a grep by name was done.
- Line-level reading of the very large `captured_returns` modules was done once; I summarised their purpose and fail-open points but did not verify every refusal path against its callers.
