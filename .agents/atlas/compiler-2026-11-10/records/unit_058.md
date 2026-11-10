# Unit 58 notes

Files covered (both under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):
1. `execution_terminal_unit_control_statement_sequence.rs` (9704 lines)
2. `execution_terminal_unit_state_graph.rs` (10120 lines)

(Notes are written incrementally while reading; see Coverage at the end for what was read.)

## File 1: execution_terminal_unit_control_statement_sequence.rs

### Part A, lines 1-5000 (read in full)

**Purpose.** Builds the "single-state body" plan: for one state of a machine it walks the admitted statement order and produces an ordered list of `CheckedUnitEffectOperationPlan`s (lets, assignments, calls, releases, cleanups) plus a structural/scalar result (header comment lines 1-23). The file is a set of nested modules: `assignments` (52-501), `bounded_byte_values` (502-617), `completion` (618-1208), `local_data` (1211-2287), `reference_retirement` (2288-...). The top-level `build` and `statement_call`, `prepare_value_operands`, `Construction`, `StatementSequence` are used by these modules through `super::` (they are defined after line 5000; recorded in later parts).

**Shape (cite file:line).**
- `assignments::Planner` 76, `AssignmentPlan {Planned, ScalarCall, StructuralCall}` 90, `assignments::plan` 107, `call_result_replacement` 381, `installed_carrier` 423, `completed_value_replacement` 441.
- `bounded_byte_values::value_plan` 525, `expression_octets` 578.
- `completion::Completion` 655, `completion::complete` 676, `forwarded_parameter` 1088.
- `local_data::Planner` 1266, `LocalPlan {Planned, ScalarCall, StructuralCall}` 1283, `plan` 1297 (wraps `plan_inner` 1312 with decline-trace restore), `dispatch_observation` 1930 (re-exported at 1209 as `pub(in crate::execution::terminal_unit)`), `PlaceLoan {Captured, Unnamed}` 2042, `place_loan` 2075, `establish_declared_home` 2216.
- `reference_retirement` (2288-): `Results` alias 2332; inner mods `completion` 2334 (impl Retirement: `completed_descriptor_sources` 2359, `call_descriptor_sources` 2460, `completed_descriptor_matches` 2515, `completion_refusal` 2571, `completion_retires_holder` 2600, `exit_cleanup_retires_holder` 2673), `prepare_case_conditions` 2839, `unique_borrow_state` 2992, `returned_reference_binding` 3011, `backing` 3035 (`backing_retired` 3043), `carriers` 3087 (`carrier_children_retired` 3115, `call_retains_carrier` 3201, `source_retains_carrier` 3215, `carrier_retired` 3231/`carrier_retired_inner` 3246, `transfers_cover_references` 3508), `owned_backing` 3549 (`OwnerCandidate` 3571, `OwnerCandidates` 3582 with `new` 3589, `prepare_owner_cleanups` 3697), `selections` 3878 (`disposes_candidate` 3900, `supplies_group` 3962, `closed_complements` 3975, `complement_completions` 4094), `stores` 4118 (`storage_source` 4133, `source_owner` 4157, `receiving_place` 4177, `destination` 4230, `receiver` 4254, `loans` 4276, `replaces` 4304, `construction` 4367, `releases` 4422), `transport` 4541 (`TransportedLoans` 4567, `collect` 4575, `supplies` 4726), `transferred_paths` 4749, `StorageBorrowers` 4777 (`new` 4782, `for_owner` 4800), `Retirement<'a>` 4815 (Copy struct of ~17 shared query fields), `RetirementNode {Loan, Carrier}` 4863, `impl Retirement`: `resource_loans` 4873, `storage_borrowers_retired` 4885, `backing_releases` 4919 ...

**Inputs/outputs.** Reads `CheckFacts` (values.structural_values, scalar_computations, flow.control, borrow ledger, contextual_cleanup, ownership), `CheckingInput` program tables (statement_table, type tables), `ShapeCollector` for type identities. Mutates the `operations` vector, `structural_results` list of `(binding, PlaceRoot)`, `scalar_count`/`structural_count` ordinals, `windows` (`OpenWindows`), `place_loans`. Everything returns `Option`; `None` means "decline to plan this body".

**How it decides (keys).**
- Assignment: first takes pre-planned stores whose `store_statement_index()` equals the statement ordinal (153-161); array assignment keyed by `scalar_arrays::assignment_type` (162); structural value keyed by `structural_values.root_for_expression(state, statement_index, expr)` (230); `root.machine != machine.symbol` -> decline (235); pending-store-order check at 318-326 declines when a pending store has `None` index or an earlier/equal ordinal; result type of call decides scalar vs structural vs unit (332-370). Unit-returning call -> `None` (361).
- Byte-vs-reference destination in `completed_value_replacement` is keyed on `byte_sequence_carrier(result_type)` and `reference_shell(declared place type)` (456-481).
- `bounded_byte_values::value_plan` requires: valid initializer, `Multiplicity::Unrestricted`, owned structural access, `BoundedOwned{capacity}` carrier (532-543); every domain constraint must have empty `establishment_routes`, default `semantic_roles`, and a byte predicate (546-553); `maximum_length > capacity` -> `None` (567). `expression_octets` is the literal-only twin (578-616), keyed on `ExpressionNode::String` (595) and `string_literal_expression_grants_domain` (608).
- `complete`: if the last op `!has_normal_continuation()` return a sequence with no result (708-724). Tail statement is the last reachable statement that is an expression and whose call does not `discards_result` (728-741). Structural return is chosen in order: returned value root (783), literal result (859), returned call (874), closed primitive array (891). `forwarded_parameter` (1088) forwards a whole formal only when value source is `Parameter` with empty path, place has no segments, same normalized type identity as `state.return_type` (1127), and for owned: access Owned, multiplicity equal, qualifications equal (1161-1166); Linear additionally compares sorted claim lists (1168-1185). Scalar completion keyed on `primitive_type_reference(state.return_type)` and exactly one authored scalar-computation root or a bound expression whose type matches (1020-1051). Final gate: `call_count == calls.len()` (1065).
- `local_data::plan_inner`, order of producers (comment 1211-1234, code 1318-1924): erased -> nothing; `dispatch_subject_of_erased_binding` -> nothing (1350); atomic uninitialised -> declared home (1354); uninit non-primitive -> deferred structural home (1379-1402); dispatch observation (1405); `RecordPattern` role -> nothing (1424); immutable byte literal (1427); bounded byte value (1465); atomic once (1488); atomic result placeholder (1512); borrowed-window move (1525); view subslice (1543-1627); reference capture (1628); place loan (1658); structural root (1675-1788); scalar array (1789); primitive (1836-1908); otherwise structural call (1909-1923) and anything not a `Call` is declined (1911-1916).
- `place_loan` (2075): `local.is_mutable` -> `None` (2084); access of reference shell must equal formation access (2089); root must be a state parameter (structural dense index, or a scalar parameter only if SharedBorrow, not const, not erased, primitive: 2109-2119) or an earlier local (2125-2128); referent type must match projected storage type unless a recast (2163-2168); a recast with an unnameable path becomes `PlaceLoan::Unnamed` (2171).
- `dispatch_observation` (1930): role must be `DispatchObservation` (1943); loan must be SharedBorrow, referee not a reference shell, no recast, path only Field/Case segments (1959-1969); following statements must all be transitions (`statement.as_transition()?` 1980); local may be named only as the single-member left side of a `CaseMembership` in a `TransitionGuard` (2005-2019); any other mention returns `None` (2005-2009).
- Retirement queries are keyed on loan handles, `PlaceRoot`, binding ordinals and statement ordinals via `StatementExecutionOrder` (`at_or_before`, `precedes`), not names.

**Cases by shape (fixed lists).**
- `place_loan` doc 2052-2074 lists admitted shapes: loan of a place rooted at a structural parameter or earlier local, projection exactly the declared referent; "A shared loan may also alias a runtime scalar parameter".
- `completion::complete` admits four structural-result shapes in an else-if ladder (783-943); anything else is `None` (941-942).
- `dispatch_observation`: transitions-only suffix and single-member case-membership guard (1980, 2011-2019).
- `PlaceLoan::Unnamed` doc 2044-2048: `self.bytes[k * 2]` style recast "lowering refuses each one".

**Same job elsewhere.**
- Array-literal local establishment appears three times in this file: assignment (162-223), completion (891-940), local (1789-1835). Same pieces each time (`scalar_arrays::elements`, `formation_domains_at`, `ScalarArray` local plan). Inferred from reading.
- The `NoNormal` branch of `prepare_value_operands` pushing `EvaluateStructuralValue` appears at 257-265, 817-836 and 1706-1714.
- `unique_borrow_state` (2992) vs the repeated "missing rows mean none, duplicates mean can't name" logic is only here; its users are all in this file (grepped later where noted).

**Fail-open risks.** (a) 1318 `local.relevance.is_erased()` returns `Planned` with nothing emitted: stated by the type (erased locals have no runtime form); traced only by name. (b) 1424 `RecordPattern` role plans nothing (stated in comment 1218 "an erased or record-pattern marker plans nothing"). (c) 1350 `dispatch_subject_of_erased_binding` plans nothing: open. (d) 2154/2171 `PlaceLoan::Unnamed` accepts a recast loan whose path no coordinate names; stated (2044-2048) that lowering refuses each use. (e) `unique_borrow_state` `[]` => `Some(None)` => callers treat as "no holders" (2992-3004, 4583-4588, 4426-4434): stated in comment 2986-2991. Other returns are `None`/`false` on any missing fact (fail-closed).

**Stated limits.** 1230-1233: "Passing such an alias onward still requires its backing storage to be established by lowering; admitting the read does not fabricate that home." 2044-2048 as above.

### Part B, lines 5001-9704 (read in full)

**Shape.**
- Rest of `impl Retirement`: `children_retired` 5051, `retains_parent` 5099, `capturing_operations` 5137 (returns `BTreeMap<(u32,u32), Vec<&Operation>>` keyed `(child.arena_index(), child.generation())`), `captures_holder` 5210, `source_captures_holder` 5331, `owner_home_established` 5416, `holder_retired` 5432, `call_receivers_retired` 5441, `holder_retired_following` 5470 (cycle guard via `active: Vec<RetirementNode>`), `holder_retired_inner` 5496, `transferred_holder_retired` 5678, `return_transfers_holder` 5860, `return_receives_carrier` 5875, `disposed_result_path` 5978, `call_cleanup_settles_owner` 6037, `disposed` 6064. Free fns `local_reference_loans` 6195, `carrier_loans` 6216.
- `mod result_consumption` 6237: `latest_loan_release_boundary` 6303, `replacement_owns_release` 6327, `normal_call_disposes_backing` 6361, `release_matches` 6451, `whole_result_discarded` 6489, `custody_transferred` 6513, `CapturedReferenceResult` 6540, `captured_storage_lifetime` 6557, `captured_reference_result` 6607, `append_reference_releases` 6753 (per statement), `append_exit_reference_releases` 7191 (at state exit), `retain_selected_sources` 7628, `consume_retained_results` 7707, `consume_value_call_sources` 7734, `consume_results` 7830, `consume_result` 7860, `prepare_value_operands` 7936, `consume_value_places` 7999.
- `mod statement_call` 8095: `Planner` 8120, `StatementCall` 8138, `plan` 8149.
- `mod storage_initializer` 8507: `value` 8530. `mod wire_codec` 8584: `written_store` 8602, `cursor_store_root` 8646, `store_path` 8672, `framed_length` 8688, `struct_literal` 8720, `field_slice_len` 8750, `slice_len` 8763, `constant_u64` 8785, `is_byte_slice` 8796, `varint_len` 8813.
- Top level: `StatementSequence` 8831 (fields: scalar_result, scalar_control, structural_result, operations, windows, place_loans, structural_local_symbols, structural_results, next_scalar_binding, owner_candidates), `finish_reference_retirement` 8870, `Construction` 8943 (10-field shared prefix), `statement_phase` 8961, `is_arm_destructure_marker` 8978, `statement_call_target` 8984, `build` 8993, `terminator_statement_index` 9464, `returning_terminator` 9489, `scalar_control` 9568, `scalar_computation_local_at` 9647.

**Inputs/outputs.** `build` (8993) takes program, facts, shapes, `PreparedCalls`, machine/state, structural and scalar parameter plans, entry claims, flow calls, a construction prefix length and a `Bound<usize>` end; returns `Option<StatementSequence>`. The retirement code mutates `operations` by `splice` at an insertion index just after the last non-call-continuation op (7172-7181, 7526-7537, 7619). It reads the borrow ledger (`facts.borrow.*`), contextual cleanup dispositions, ownership selections and `StatementExecutionOrder`.

**How it decides.**
- `build`: `scalar_control` decides whether the state ends in a scalar terminator (9013-9016). `sequenced` (9018-9048) marks a statement as outside the walk when it is a non-terminal-call `Transition`, an arm-destructure marker, or a value-returning completion expression (non-Call that may complete normally, or one that has a Return scalar computation root or a structural value root). The walk stops at the first unsequenced statement; the rest must be owned by completion: `completion_owns_rest` (9077-9085) accepts (a) position at or after the scalar terminator, (b) the last statement being one `Expression`, (c) only Transitions after skipping arm markers; otherwise "unsupported statement kind" -> `None` (9086-9094). Per statement it dispatches on `StatementNode` kind (9244-9398): `Assignment` (atomic carrier or `assignments::plan`), `LocalData`, `Call` (discards_result variant 9316), `Expression` (completing scalar / structural / tail call / unit call), `Transition` with terminal invocation; every other kind `_ => return None` (9398). `Assembly`/`AssemblyFact` and fact-only citations are skipped (9220-9228).
- `statement_call::plan`: exactly one flow call with `call.statement_index == index` else `None` (8193-8204); competing -> `None`; for a transition it must be the terminal occurrence (8205); the authored call's `target_symbol` must equal the flow fact's (8221-8227). Atomic fence calls are chosen by `crate::checks::atomic_fences::contract` (8256) and must have no result (8258). Wire-codec call chosen by `validation::compiler_operations::statement_wire_codec_intrinsic` (8334).
- `wire_codec::written_store`: keyed on spelling, `call.target.as_str() != "encode"` (8609); schema found by `schema.name.as_str() == leaf.as_str()` where leaf is last receiver path member (8698-8701); field looked up by `supplied.name.as_str() == field` (8759).
- `holder_retired_inner` (5496-5676) is the central judgment, tried in order: loan not yet formed before current statement -> retired (5502); `disposed` (5508); at exit and the return transfers it (5511); descriptor-load observation compares release boundary (5514); at exit with `LastUseExpired`/`StateExit` weakening and established owner home (5527-5543); `LocalReassigned` weakening where a call replaces the loan (5544-5566); otherwise transfers (`transfers::from_storage`) then `completion_retires_holder` (5660-5675). Keyed on loan handles, binding ordinals, statement ordinals, `PlaceRoot`; never on names.
- `consume_result` (7860-7921): keyed on producer's multiplicity and consumer access. `MoveStructuralField` producer => ok without disposal change (7875); `Unrestricted` => ok (7878); reference carriers: Owned only when not projected, borrows always ok (7889-7897); borrow access on others => ok (7898); Affine owned whole => `discard_result_on_return = false` (7917); Linear needs `!discard && Owned && !projected` (7908-7914); else `None`.
- `consume_value_places` (7999) walks `CheckedStructuralValueKind` with an explicit match; leaf kinds listed at 8080-8089 do nothing. A node reached twice (`visited.contains`) returns `None` (8009).

**Cases by shape.**
- `build` `completion_owns_rest` three shapes (9077-9085) and "unsupported statement kind" (9087).
- `wire_codec` (comment 8584-8596): only `Schema::encode`; fields must be byte slices indexed with constant exclusive ranges (`range.end_inclusive` -> None 8779; `Indexed` whose index is `Range` 8771-8778; `Integer` literal only 8789); any non-byte-slice field or `Version` member -> `None` (8714); struct literal must be the initializer of a local in the same state (8733-8743) or a borrow of it (8725-8728); store path only `Field` segments (8678); root only `Local` or `Parameter` symbol whose `parent == state.symbol` (8652-8668).
- `returning_terminator` (9489): binary terminator `Conditional|Return|Crash` else at most one guarded tail whose arms are all `Return` and whose first arm guard ordinal equals prefix count (9522-9528); returning tail only for Return-transition, Guarded, Conditional-with-two-Returns, Crash (9551-9564).
- `scalar_control` (9568): requires a scalar (primitive) return type (9574); requires retained return expressions for every destination (9601-9622); `Crash` counts as complete (9620).
- `storage_initializer::value` (8530): exactly one of computation root or bound pure expression; scalar short-circuit boolean rejected (8576-8577).
- `varint_len` (8813) errors above 10 bytes.

**Same job elsewhere.**
- `build` (8993) is the "single-state body producer" and is called from two sites outside this file: `execution_terminal_unit_control.rs:5017` and `execution_terminal_unit_state_graph.rs:9839` (grep `statement_sequence::build`, both read as hits only). `finish_reference_retirement` likewise called at control.rs:5037 and state_graph.rs:9908.
- Per-statement retirement (`append_reference_releases` 6753) and exit retirement (`append_exit_reference_releases` 7191) are two near-parallel loops over `captured_reference_result` with the same skip tests (discarded, replacement_owns_release, custody_transferred) and the same `ReleaseReference {...}` construction (6914-6926 vs 7331-7343). Read.
- The `Retirement { ... }` struct literal with 17 fields is written out by hand at 3800-3819, 6797-6816, 7360-7379 (and variants). Read.
- Release/boundary arithmetic in `state_graph.rs:4425,4441` calls `captured_reference_result` and `release_matches` from this file; suspected repeated retirement logic there (not read yet at this point; see File 2 notes).
- Scalar-computation selection "exactly one root, authored root matches, no bound expression also present" appears at 1020-1051 (completion), 8542-8563 (storage_initializer) and 9662-9695 (scalar_computation_local_at). Read.

**Unused or one-caller code (grepped across `omega-rust`, Grep content mode over `*.rs`).**
- `build`: 2 external call sites (control.rs:5017, state_graph.rs:9839).
- `finish_reference_retirement`: 2 external (control.rs:5037, state_graph.rs:9908).
- `dispatch_observation`: 1 external (state_graph.rs:192) plus the in-file use 1405.
- `release_matches`: 1 external (state_graph.rs:4441) plus 3 in-file.
- `captured_reference_result`: 3 external (cleanup.rs:4971, state_graph.rs:4425, plus in-file uses).
- `consume_retained_results`: 3 external (control.rs:6258, execution_terminal_unit.rs:6494 import and 6807 call).
- `consume_results`: 4 external (state_graph.rs:908, 7976, 8008; control.rs:6879). `consume_result`: 1 external (execution_terminal_unit.rs:657 import).
- `prepare_value_operands`: 1 external (control.rs:6776).
- `scalar_control`: externals execution_terminal_unit.rs:3744, state_graph.rs:5824, control.rs:3844. `returning_terminator`: control.rs:5099. `terminator_statement_index`: state_graph.rs:9804.
- `retain_selected_sources` (7628): only the in-file call at 7985 (the other grep hits are an unrelated `SourceMap` method of the same name).
- `append_exit_reference_releases`: only called at 8907 in this file; `append_reference_releases` in-file at 9204 and `captured_reference_result` as above.
- `build` parameter `_call_frames: Option<&validation::CallFrameResolver<'_>>` (9009) is underscore-named and unused in the body (read).
- Comment 8932-8942 says `Construction` is the one owner of the shared ten-field prefix for "the four producers". Users outside this file: grep `statement_sequence::Construction` found none (`pub(in super::super)` visibility); inside the file it is used by `assignments`, `local_data`, `statement_call`, `completion` (4 producers, matches the comment).
- `capturing_operations` returns a `BTreeMap` (5137) keyed by `(arena_index, generation)`, a handle map using ordered map rather than a dense vector (relevant to `tools/dense_id_maps.txt`; not checked against that list). Inferred.

**Stated limits (quotes under 20 words).**
- 8595: "An open selector or a field this walker cannot size fails the plan." (wire codec).
- 8869-8868 comment: retire descriptors "only after the consumer has produced this state's final value".
- 7628 comment: selected continuation owns "both the chosen result and the conditional complement until the actual death edge".
- 6320-6324: replacement-owned carriers are not released by statement-entry expiry (migration of ownership; not marked debt).
- No TODO/migration-debt wording found in this file (Grep for `TODO|migration|unsupported` was not run; the only "unsupported" string is the trace at 9087 and the read comments above).

**Fail-open risks.**
- 5385/5390 (`source_captures_holder`): if `storage_owner_type_reference` or `loan_place_segments` returns `None` it returns `true` ("source does hold the parent"). Direction: makes a child retain its parent, which blocks retirement; so a missing type fact keeps a loan alive longer (more conservative), not accepted. Traced by reading callers (`retains_parent` / `captures_holder` feed `children_retired`, which must be all-retired to release).
- 5406: `BorrowLoanOwnerSegment::DynamicIndex` matches any selected segment (`=> true`) in the same function: same conservative direction. Read.
- 5502-5506: a loan whose formation statement does not precede the current statement counts as already retired. Wrong program that would get through: none identified, since such a loan is not yet formed; no comment states this. "open".
- 7230-7232: `!prepared_completion.has_normal_completion()` returns `Some(())` with no release ops (a state that never completes normally needs none). Not stated beside the site. "open".
- 7208-7221 and 6770-6783: no loans and no captured reference results => `Some(())` (nothing to retire). Reasonable by construction; "open".
- 8201: `StatementNode::Call` without a flow call returns `None` (fail-closed). 8254: after operand planning, a non-returning op ends the statement early `Some(())`; the sequence then reports no normal continuation (read).
- 9224-9228: `is_fact_only_citation` statements are skipped with no operation (`crate::execution::citation_erasure`, not read); "open".
- `completion::complete` 708-724 returns a result-less sequence when the last op is non-returning; stated by comment 622-623.
- `local_data` `PlaceLoan::Unnamed` as in Part A (stated).
- No site was found that replaces a missing fact with "accept" apart from those listed.

## File 2: execution_terminal_unit_state_graph.rs

### Part C, lines 1-6500 (read in full; later parts below)

**Purpose.** Plans a state machine as a "composed Unit control graph" (header 1-13): fixes result signature and ranks, plans each state's prefix and statement sequence (delegating to File 1's `build`), admits operations, plans the terminator (jump, conditional, guarded, closed sum, return, crash), accounts every structural result's custody, then checks parameter custody and provider attachments. The file is a set of sibling modules glued together by `build_traced` (defined after line 6500; see later parts).

**Shape (file:line).**
- `mod closed_sum` 41-926: `build` 85 returns `CheckedComposedUnitControlTerminatorPlan::ClosedSum` (623); `DispatchSubject {Place, Computed}` 638 with `is_symbol` 659, `names` 673, `payload_field` 701; `subject_call` 790.
- `mod fused` 927-1174: `FusedStates` type 946, `fused_states` 958, `moves_activation_claim` 1080, `completing_transition_targets` 1094, `completing_transition_transfers` 1101, `shares_borrowed_storage` 1123, `fused_target_index` 1161.
- `mod guards` 1175-1275: `checked_qualified_arrivals` 1199, `retained_guard` 1235, `guard_is_boolean` 1256.
- `mod loan_backing` 1276-3455: `record_selections::RetainedBorrowRecords` 1318 (wraps `KeyedRows`), `DeclaredStoragePlans` 1556 (`symbols::SymbolKeyMap` of carrier -> `Option<Arc<[Vec<BorrowLoanOwnerSegment>]>>`), `selected_call` 1732, `arguments_name_call` 1758, `RetainedStorage` 1788, `retained_storage` 1798, `local_backing_binding` 1992, `local_storage_binding` 2024, `retained_parents` 2075, `shared_leaf_lifetime_roots` 2322, `ingress_covers_capture_path` 2364 (+`_inner` 2385, `_body` 2409), `unchanged_ingress_capture` 2525, `owned_parent_source` 2558, `retired_carrier_backing` 2615, `StorageQuestion {RetainForArrival, ProveRelocationIndependence}` 2826, `Dependencies<'a>` 2831 (`captured_storage` 2849, `selected_path` 2949, `root` 2982, `contents` 3000, `contents_at` 3010, `invocation_lifetimes` 3126, `loan` 3188). In-file unit tests at 1375-1545 and 1616-1730.
- `mod local_results` 3457-3596: `permits_disposal` 3469, `validate` 3492.
- `mod operation_custody` 3597-3856: `admit` 3619, `call_custody_admitted` 3774, `call_argument_admitted` 3801, `call_custody_refusal` 3827.
- `mod ranking` 3857-4181: `natural_rank_plans` 3872 (`pub(crate)`, re-exported at 5752), `natural_rank_plans_for_fused` 3884, `bind_observations` 4097, `complete_witness_ranks` 4140.
- `mod result_custody` 4182-5751: `account` 4229 (about 960 lines), `producer_claim_paths` 5196, `disposed_partition` 5240, `view_loan_releases` 5288, `view_formation_claims` 5452, `installed_view_claims` 5501, `construction_local_uses` 5598, `terminator_call_consumes_result` 5682, `call_owns_result` 5700, `retained_structural_result` 5721, `operand_owned_construction` 5740.
- `pub(super) mod returns` 5754-6129: `scalar_completion` 5789, `guarded` 5838, `next_result_ordinal` 5929, `return_value_operation` 5951, `literal_result_operation` 6086.
- `mod successors` 6130-...: `SuccessorEdge` 6166 (Jump, Conditional, GuardedJump, ClosedCase), `SuccessorGuard` 6174 (16 trace-only labels), `EdgeCalls` 6259, `successor` 6268, `successor_bindings` 6322 (continues past 6500).

**Inputs/outputs.** Reads `CheckFacts` (flow.control calls/operands, borrow ledger, ownership/permission events, contextual_cleanup dispositions, values.structural_values, scalar expressions/computations), `CheckingInput` tables, `ShapeCollector`. Produces `CheckedComposedUnitControlTerminatorPlan` variants, `CheckedStructuralControlSuccessorPlan`, `RetainedStorage`, rank vectors. `operation_custody::admit` and `result_custody::account` mutate the operations slice (`discard_result_on_return` flags: 3636, 5187).

**How it decides.**
- `closed_sum::build`: guard must be `When` with `exact_outcome_case_test` (111-114); subject taken from a place loan (178-224), a state parameter (225-268) or an earlier immutable local result (269-325), else a computed call subject (144). Variants come from `closed_data_definition(type_reference)` minus `impossible_variant` (373-382). Each transition is `When(case)` for one variant, or a final `Always` taking every not-yet-covered variant (393-425); duplicate case identity -> `None` (537-542); `cases.len() != variants.len()` -> `None` (620). Source spelling compared in `payload_field` Computed branch: `name.as_str() != variant.name.as_str()` (766-769) and `field.name.as_str() == member.member.as_str()` (774).
- `fused::fused_states`: starts from the machine's own states, adds states reachable via completing transition targets whose owner `body_is_present` (970-989), computes SCCs over owners (1020), and keeps only the entry component plus components reached by a tail transfer that `moves_activation_claim` (1054, keyed on `type_carries_activation_claim`) or `shares_borrowed_storage` (1055; two reference-shell arguments with overlapping places, at least one not Shared, 1144-1151).
- `loan_backing::selected_call` (1732): flow calls at the statement whose `authored_site` is `NamedTransition`, or `Expression` equal to the transition-value/expression statement; exactly one match else `None` (1754-1755). `arguments_name_call` (1758): same target and identical argument span, or dynamic slot whose `unique_dispatched_entry` equals target with `authored[1..] == arguments` (1773-1781).
- `Dependencies::loan` (3188-3453) keys on `BorrowLoanLineage` (`Reborrow`, `DirectRoot`, `SuppliedOutput`, `LifetimeDependency`, `UnretainedDerived`: 3415-3439) and `IncomingReferenceOrigin` (`Parameter`, `Output`, `Invocation`, `DescriptorLoad`) plus `StorageQuestion`.
- `ranking::natural_rank_plans_for_fused` (3884): no `implementation_witness` on any owner and no runtime components -> empty result (3892-3897); otherwise requires each judgment `local_terminates` (3910) and a rank for each state where `relevance.requires_rank` (3915); component states/edges must all be inside the fused roster (3950-3960); measure compatibility is a fixed match of `(RuntimeRankInput, CheckedNaturalRankMeasure)` pairs: Value/DeclaredMeasure, Value/IntegerValue, Value-Length/SequenceLength, Distance/Distance, else `false` (3975-4058).
- `result_custody::account` (4229): per producer result, counts dispositions (implicit via terminator, call transfers, cleanup discards, construction transfers, reference releases, view releases) and requires `total == 1`, or `total == 0 && !has_normal_caller_completion` (5132). Keyed on binding ordinals, occupants, `PlaceRoot`, and permission event kinds/claim identities, never names. Terminator dispatch `match terminator` at 4852-5082 lists every `TerminatorPlan` variant; `Crash | InvokeNoNormal | EvaluateNoNormal => false` (5079-5081) so custody is not claimed there but `total == 0 && !normal completion` can pass.
- `returns::literal_result_operation` (6086): byte literal via `expression_octets`, or a string literal granted view domains when the return type is a borrowed slice view (6101-6112).

**Cases by shape.**
- Closed sum: `statements.len() - transition_start > variants.len() + 1` -> `None` (384); only `When`/final `Always` guards (393-425); payload forwarding only for primitive-typed fields (573); parameter subject must be Owned or shared/mutable borrow, multiplicity Affine or Unrestricted, no qualifications, no `primitive_referent` (235-255); `WriteOnlyBorrow` -> `None` (246); a place-loan subject must be SharedBorrow and unique (182-188); a `ScalarParameter` loan root -> `None` (213).
- `returns::guarded` (5838): not Unit return, not Linear (5852-5855); exactly one guarded tail whose first arm guard ordinal equals `start` (5857-5870); every destination must be `Return` (5881-5887).
- `returns::scalar_completion` (5789): refuses a state whose remaining statements contain a fused completing transition (5802-5814).
- `fused_states` comment 951-957: tail transfers stay fused only for recursion, activation custody or overlapping borrowed storage; "Other acyclic calls keep their ordinary operand preparation".
- `operation_custody::admit` (3639-3762): fixed list of admitted `OperationPlan` variants; the `_ =>` arm names the kind in the trace and returns `None` (3723-3761).
- `disposed_partition` (5240): only `FixedArray` shapes with literal length may be covered by their elements (5253-5265).
- `result_custody::account` `selected_case_exhausted` (4986): "A union of alternative moves cannot exhaust one selected case" (4988) excludes `Completion` arms.
- `successors::successor_bindings` (6322, partly read): `transition.exit != Ordinary || continuation.is_valid()` -> `None` (6344); argument count must equal target parameter count (6355); structural transfer attempts in fixed order: parameter, byte-subslice, string literal, place/result (6396-6500...).

**Same job elsewhere.**
- Retirement/release scheduling here (`result_custody::account`, `view_loan_releases` 5288) overlaps File 1's `Retirement` machinery (`captured_reference_result` and `release_matches` are called from here at 4425 and 4441; defined in File 1 at 6607 and 6451). Read both.
- `fused::fused_target_index` 1161 normalizes a named target to a state index "the same normalization `activation_target_state_index` applies" (comment 1157-1159). Suspected: grep `activation_target_state_index` below.
- Scalar guard retrieval: `guards::retained_guard` (1235) and File 1 `scalar_control`'s `retained_return` (9576) both read `scalar_expressions.expression_at` / `scalar_computations.root_at` for a role and statement. Read.
- Result type + `result.statement_index/binding_ordinal` setup repeats at 6030-6052 here and at File 1 839-842, 1491-1494, 9172-9176.
- `local_results::validate` (3492) and `result_custody::account` both resolve "the producer root, occupant and exit" using `cleanup::normal_edges::producer_root` and `local_storage::occupant` (3504-3507, 4322).

**Unused or one-caller code (greps pending where noted).** `RetainedStorage` and `retained_storage` are `pub(super)`; their users are in later parts. `natural_rank_plans` re-exported `pub(crate)` at 5752: see grep below (Part D).

**Stated limits.**
- 1196-1197: "A decision may retain a constant obligation. This permits planning only; machine lowering refuses pending obligations".
- 1284-1285: "Unresolved coverage retains the stable-home route."
- 2073: "A dependency is storage lifetime only: cleanup still needs its own action."
- 4205-4206: "covering a nominal owner's fields still cannot discharge its own cleanup."
- 5497-5499: destination access and replacement cleanup "remain the store's obligations, not permission granted by this accounting join".

**Fail-open risks.**
- 1898-1903 (`retained_storage`): when no root `LoanResource` exists for the operand and the operand type has no `stored_loan_parts`, it returns `Ok` with no backing. A call whose operand carries no descriptor needs none; a type with descriptors and no resource is refused (1896). Traced by reading.
- 2000-2002 and 2566-2568, 3021-3023: image storage roots return `Ok(None)` / `true` (static image storage is not a local home). "open" (no comment at site; `is_image_storage_root` defined in `crate::borrow::accesses`, not read).
- 3802-3807 (`call_argument_admitted`): a non-Owned argument, or an Owned argument whose source is not a structural result, is admitted unconditionally. Stated at 3795-3799: "The call sequencer already joined each operand to its loan or consumption."
- 3568-3570 (`local_results::validate`): `Unrestricted` multiplicity returns `Some(())` with no cleanup check (unrestricted values need no disposal). "open".
- 3571-3581: `no_code_completion_owner` plus no-code-cleanup contents plus borrow-free accepts with no disposition row. Stated by name only. "open".
- 3895-3897 and 3905-3909: a graph with no implementation witness and no runtime components, or an owner with no witness and no observations, gets no rank. Depends on checking having decided termination elsewhere; not traced. "open".
- 5132: `total == 0 && !has_normal_caller_completion` passes custody accounting with zero dispositions (stated at 4190-4191: "A domain-ending or all-crash exit abandons custody without manufacturing a disposition").
- 3436-3438 (`Dependencies::loan`): `UnretainedDerived => incoming || !loan.supplying_loans.is_empty()`; 3433-3435 `LifetimeDependency` true only for `RetainForArrival`. For `ProveRelocationIndependence` the answer is stricter. Traced by reading.

### Part D, lines 6501-10120 (read in full)

**Shape (file:line).**
- `successors::successor_bindings` 6322-7106 (body at 6338-7106): per target structural parameter, one closure tries transfer shapes in fixed order, then retained storage, scalar, erased and proof arguments. Helpers: `projected_value_subject` 7114, `transition_selector` 7228, `structural_transfer_plan` 7261, `projected_loan_transfer` 7291, `case_payload_transfer` 7415.
- `mod terminal_calls` 7580-8027: `invoked_callable` 7641, `external_callable` 7664, `completion` 7685, `arm_call` 7759, `plan_call_arm` 7841 (also called from `unit_completions` 9380).
- `mod terminator` 8028-9311: `build` 8082 (about 840 lines), `internal_destination` 8934, `guarded_return_arm` 8987, `covering_pair` 9105, `crash_arm` 9128, `complete_selected_return` 9147, `erased_proof_completion` 9255, `transition_on_erased_dispatch_subject` 9275, `expression_names_erased_dispatch_subject` 9286.
- `mod unit_completions` 9312-9585: `build` 9327, `bind_expression` 9446.
- Re-exports 9587-9594; `build_traced` 9597-10115; `type Signature` 10117.

**Inputs/outputs.** `build_traced` (9597) takes `CheckFacts`, `RetainedBorrowRecords`, `ShapeCollector`, `PreparedCalls`, the machine and (unused downstream) `call_frames`; returns `Option<CheckedComposedUnitControlMachinePlan>` via `composed_control::finish_state_graph` (10083). It composes, per fused state, the signature (9677-9786), then `control::statement_sequence::build` (9839), `operation_custody::admit` (9859), `terminator::build` (9860), window closure check (9883), `prepare_state`, `finish_reference_retirement` (9908), `consume_caller_results`, `result_custody::account` (9946), then pushes a `CheckedComposedUnitControlStatePlan` (9962).

**How it decides.**
- `build_traced` early gates: no states or machine has binders -> `None` (9611); a scalar-result machine may carry only `Requires`/`Crashes`/`Ensures` contracts on state 0 and only `Requires`/`Crashes` on others, and `contract.binding.is_some()` is refused (9616-9642); each state's `declared_result_type` must equal the machine result (9680); `structural_state_contract_predicates` must be admitted (9684-9689); a `SourceFact` contract row is refused for the first state (`signatures.is_empty()`) or when no `state_requires_fact` exists (9693-9706). Predicate lowering failing with no source expression -> `None` (9746-9751).
- `operation_end` (9809-9825): `Included(terminator_index)` when the terminator statement is an Expression with a structural root, an Expression that is a `Call`, or a terminal invocation outside the fused set; else `Excluded`.
- `terminator::build` order (comment 8030-8043): no-normal call/evaluation first (8104-8121); selected static destination for a first `When` (8224-8276); Unit completion (8278-8318); closed sum (8320-8338); scalar completion (8341-8355); guarded (8356-8371); else a match on `reachable_transition_prefix` (8373): `[]` Unit, `[Expression]` non-Unit, `[Transition Crash]`, `[Transition Always]` with sub-cases Terminal / fused Value / terminal call / Value completion / default Jump (8391-8467), `[When, X]` where X is `Always`, `complementary(...)` or `covering_pair(...)` (8468-8479), a chain of 3 or more transitions (8686-8703), `_ =>` unsupported tail (8876-8921).
- `covering_pair` (9105): exactly one retained guarded tail for the state, no fallback, two exits at consecutive ordinals (9113-9121). Comment 9100-9103: "a retained pair records only when its guards provably close the dispatch".
- `internal_destination` (8934): crash must be exactly at the ordinal (8942-8945); a fused target state wins (8949-8953); otherwise keyed on `CheckedScalarBranchDestination` variant and ordinal equality (8954-8973).
- `expression_names_erased_dispatch_subject` (9286): follows at most 8 hops through `Borrow`, `Member`, `Binary.left`, `Unary` to a `Name` and asks `dispatch_subject_of_erased_binding`; `SavedSubject` and any other node -> `false` (9305-9306); after 8 hops `false` (9309). `transition_on_erased_dispatch_subject` returns `true` for any non-`When` guard (9280-9282).
- `terminal_calls::completion` (7685): exactly one call at `(statement_ordinal, call_ordinal 0)` (7692-7702); pairs `(CheckedControlResultPlan, CheckedUnitCallResult)` in fixed combinations Unit/Unit, Scalar/Scalar (primitive, statement and bound all equal), Structural/Structural (statement, type identity, multiplicity equal and returned source equals the call binding); `_ => None` (7744).
- `terminal_calls::arm_call` (7759): unique non-retired flow call at the statement whose `find_call_site(...).completes_transition` and `transition_call_target == transition.target` (7790-7815).
- `unit_completions::bind_expression` (9446): scalar dispatch arms only in four `(pattern, comparison)` pairs: Value/Boolean, Value/Integer, Predicate/Predicate, Wildcard/Predicate (9482-9496); structural arms only Predicate/Predicate and Wildcard/Predicate (9525-9535); a cycle (`active.insert` fails) or `Kind::Missing` -> `None` (9460, 9465).
- Transfer shapes in `successor_bindings` closure (6393-6901): (1) parameter forwarding when type identity, access and normalized types match (6396-6416); (2) byte subslice with `Range` index (6417-6434); (3) string literal, only SharedBorrow Unrestricted byte view (6435-6463); (4) place/result transfer for Owned, shared view or mutable view (6473-6525); (5) case payload (6527-6544); (6) projected value (6545-6562); (7) loan transfer for borrowed targets (6583-6757); (8) established evaluated argument (6758-6900); else `None` (6901).
- `case_payload_transfer` (7415): target Owned with non-Unrestricted multiplicity -> `None` (7426); the place must be exactly the subject place plus `[Case(selected), Field]` (7431-7448); Owned needs field contents `NumericOwned` or `OwnedOrSharedView` plus equal normalized identity (7470-7482); borrowed target needs SharedBorrow of a shared reference field (7483-7492); field multiplicity must equal target multiplicity (7493). Doc 7403: "Admit a `[copy]` case-payload subtree transfer".

**Cases by shape.**
- Unsupported tails list (quoted keys, 8878-8915): missing return value; expression statement with unit result; single guarded transition; single guarded crash exit; jump followed by a transition; guarded pair without exact false fallback; transition chain; transition followed by statements; expression followed by statements; other statement tail. All `return None`.
- `projected_loan_transfer` path segments allowed: `Field`, `Case`, `FixedIndex`, `RuntimeIndex(Parameter|Local)`, `ComputedIndex`, `Referent` (7330-7342); `FixedArraySlice` presentation refused (7351).
- `projected_value_subject` (7114): Owned targets only (7127); path non-empty and only Field/FixedIndex/Case (7196-7204); `WriteOnlyBorrow` root refused (7182); non-Unrestricted multiplicity needs Owned root (7206-7207).
- `transition_selector` (7228): primitive must be `is_fixed_integer()` (7249).
- `build_traced` live-state loop (10001-10081): qualified-arrival check (10014-10021); Owned parameter must have cleanup/represented/construction contents, a view carrier, or be a reference record unless Linear (10043-10063); borrowed parameter must be Unrestricted and registered in `shapes.types` (10066-10077).
- `fused_states` / `fused_target_index` (1161): a state symbol resolves directly, or a machine head resolves to its entry state via `validation::find_machine_head`.

**Same job elsewhere.**
- `fused::fused_target_index` (1161-1173) re-does the "named target to state index" normalization that `validation::activation_target_state_index` does (comment 1157-1159). grep `activation_target_state_index` across `omega-rust` (Grep content mode, uncapped): defined `validation/src/call_cycles.rs:3285`; used in about 18 other places in 11 files (`flow.rs`, `flow_transfers.rs`, `flow_state_values.rs`, `values_scalar.rs`, `execution.rs` x3, `execution_scalar_computations.rs` x2, `checks_contracts_exits.rs`, `published_facts_crash_entry_values.rs` x4, `published_facts_boolean_predicates.rs` x4). `fused_target_index` is used inside this file only (`pub(super)` re-export at 9588; grep hits within this file). Read for this file, grepped for the others.
- Scalar `Return`-role retention test `expression_at(...).is_some() || root_at(...).is_some_and(|root| root.machine == machine.symbol)` is written out three times in this file (closed_sum 473-482, terminator 8588-8597, terminator 9058-9067) and in File 1 `scalar_control::retained_return` (9576-9592). Read.
- `closed_sum::build` arm return logic (462-531) and `terminator::build` conditional `branch` closure (8496-8647) and `guarded_return_arm` (8987-9097) each do "Value target -> scalar-retained check, Unit -> `unit_completions::build`, structural -> `returns::return_value_operation`". Read.
- `crash_arm` (9128) and the crash handling in `internal_destination` (8942-8945) / `guarded_return_arm` (9004-9006) agree on "Crash with Terminal target and no continuation".

**Unused or one-caller code (Grep content mode, uncapped).**
- `build_traced`: 1 external call, `execution_terminal_unit.rs:8096` (`BodyRoute::Graph`).
- `natural_rank_plans` (`pub(crate)`, 3872): 2 external callers: `execution_scalar_computations.rs:5349`, `execution.rs:1364` (import at 1343); the same-file path is `natural_rank_plans_for_fused` (called at 9660).
- `checked_qualified_arrivals`: 1 call (10016, this file).
- `retained_guard` / `guard_is_boolean`: used inside this file (terminator, result_custody); no external use matched (`state_graph::retained_guard` pattern: 0 hits outside).
- `result_custody::terminator_call_consumes_result` (5682): 1 call (4414). `call_owns_result` (5700): 1 call (3820). `operand_owned_construction` (5740): 5 calls (3627, 6496, 6618, 7514 and the import).
- `returns::literal_result_operation` (6086): 1 external call, File 1 line 860, plus import at 639 and the in-file use at 5966. Documented 6079-6085 as "this one producer" for a literal return: users outside its file: 1 (File 1). So the claim "A body's final expression and a returning state's value arm complete a literal through this one producer" holds with 2 users (File 1 860, this file 5966).
- `terminator_statement_index` (File 1 9464): used here at 9804.
- `SuccessorGuard` has 16 variants whose only use is spelling a trace phase (6193-6247). Read.
- `build_traced` passes `call_frames` (9855) into File 1 `build`, whose parameter is `_call_frames` (File 1 9009), so the argument is unused downstream. Read.

**Stated limits (quotes under 20 words).**
- 8038: "an unsupported tail names its shape." (header, 8038)
- 8039-8042: normal returns "do not independently require whole-root no-code cleanup" - `cleanup::normal_edges` owns it afterward.
- 9440-9441: "A union of leaf outcomes supplies no abandonment proof" so `abandons_custody: false`.
- 9100-9103: covering pair "records only when its guards provably close the dispatch".
- 9562-9563: "Operand execution can stop before a normally returning callee."
- No `TODO`/`migration debt` wording seen in this file.

**Fail-open risks.**
- 8482-8487 and 8704-8713: when the result is Unit and every transition in the tail is on an erased dispatch subject (or an `Always`), `erased_proof_completion` returns `ReturnUnit` without planning any arm (9255-9269). Wrong program that could get through: a Unit state whose guards name only erased proof bindings but whose arms perform effects; the comment at 9251-9254 says "the arms are not executed" and 9272-9273 says "The subject is proof identity." Stated; whether arms can carry runtime effects is not traced here. `transition_on_erased_dispatch_subject` returning `true` for non-`When` guards (9280-9282) means an `Always` arm is accepted with no check of what its target does. "stated" for the intent, "open" for effects.
- 7371-7372 and 7362-7367 (`projected_loan_transfer`): when `shapes.add_reference_type` fails and the path has no dynamic selector, the carrier identity becomes `Default::default()` and the plan proceeds; bounded byte views skip the carrier identity entirely. No comment at the site. "open".
- 7864-7866 (`plan_call_arm`): `normal_completion_possible(...) == Some(false)` is the only no-normal case; `None` (unknown) is treated as "has a normal result". Conservative direction (plans a result binding). Traced by reading.
- 3895-3897 (carried from Part C): no witness and no runtime components -> empty ranks.
- 9117-9121 `covering_pair` returns `Some(())` iff retained facts say the two guards close; relies on the checker having recorded that (stated at 9100-9103).
- 10002-10004: states not in `live_states` skip parameter-custody checks (unreachable states). Traced by reading: `live_mask` defined in `CheckedComposedUnitControlStatePlan` (not read).
- 10015: `state_index == 0 && invocation_only_entry` skips `checked_qualified_arrivals` for the entry state when no live successor targets it; stated by the name only, with the guard comment at 1191-1193: "Invocation-only entry is handled by its caller".
- Otherwise the code returns `None` on any missing row.

## Coverage

- `execution_terminal_unit_control_statement_sequence.rs` lines 1-9704 (9704 lines): read in full.
- `execution_terminal_unit_state_graph.rs` lines 1-10120 (10120 lines): read in full.
- Skimmed: none. Not read: none. Total 19824 of 19824 lines read.
- Greps run (Grep tool, content mode, whole `omega-rust`, uncapped via `head_limit 0` on the state_graph/activation ones): `terminator_statement_index|returning_terminator\(|...` (count mode), `finish_reference_retirement|append_exit_reference_releases|retain_selected_sources|consume_retained_results|release_matches\(|dispatch_observation\(|statement_sequence::build`, `statement_sequence::(scalar_control|returning_terminator|...)`, `natural_rank_plans\b|build_traced\(|activation_target_state_index|fused_states\(|FusedStates`, `state_graph::(retained_structural_result|...)|terminator_call_consumes_result|call_owns_result|operand_owned_construction`. Hits in other files were not opened; the other-file sites are "suspected" unless stated.

## Five things a maintainer most needs to know about this unit

1. The single-state body producer (File 1 `build`, 8993) and the composed-graph planner (File 2 `build_traced`, 9597) are two callers of one pipeline: File 2 calls File 1's `build` (File 2 9839) and `finish_reference_retirement` (File 2 9908); File 1's `build` also has a second external caller in `execution_terminal_unit_control.rs:5017`. Both files plan the same statement kinds, but terminators exist only in File 2.
2. "Is this loan/holder finished?" is answered by one 17-field `Retirement` struct and about 25 methods in File 1 (4815 onward); every decision is keyed on borrow-ledger handles, binding ordinals, statement ordinals and `PlaceRoot`, never on names. Missing facts return `false`/`None`, so retirement scheduling fails closed; the exceptions are `source_captures_holder` returning `true` on missing type facts (File 1 5385-5390), which also keeps things alive.
3. File 2's `result_custody::account` (4229-5190) enforces "exactly one disposition per structural result" by counting six kinds of disposal; `total == 0` passes only when there is no normal caller completion (5132). It reads File 1's `captured_reference_result` and `release_matches`.
4. Program shapes are enumerated as fixed lists: transition tails (File 2 8373-8921, unsupported ones named at 8878-8915), successor transfer kinds (6393-6901), unit-completion dispatch patterns (9482-9535), statement kinds in `build` (File 1 9244-9398) and the wire-codec `encode` form (File 1 8584-8824, keyed on the spelling `"encode"` at 8609 and on schema/field names at 8701, 8759).
5. Source spelling is used for judgments in only a few places: File 1 `wire_codec` (8609, 8701, 8759) and File 2 `closed_sum::payload_field` Computed branch (766-774). Everything else keys on symbols/identities.

## What I am unsure about

- Whether the erased-proof completion (File 2 8482-8487, 9255) can accept a Unit state whose `Always` fallback arm has runtime effects; I did not trace `dispatch_subject_of_erased_binding` or `selected_transition`.
- Direction of `source_captures_holder`'s `true` fallbacks (File 1 5385, 5390): I read callers `captures_holder` -> `retains_parent` -> `children_retired`, and concluded `true` only delays retirement; I did not read the later consumers of `children_retired` in other files.
- Whether `is_image_storage_root` (File 2 3021 etc.) is correct for all roots; not read (`crate::borrow::accesses`).
- Whether `fused_target_index` and `activation_target_state_index` are interchangeable; I did not read `validation/src/call_cycles.rs:3285`.
- Line-range citations in Part A/B for `Retirement` methods are by reading start lines; a few helper boundaries (for example where `mod selections` ends) are approximate to a few lines.


