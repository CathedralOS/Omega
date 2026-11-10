# Unit 61 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`):
1. flow_mutation.rs (1-4762)
2. flow_observations.rs (1-1690)
3. flow_operand_contexts.rs (1-1136)
4. flow_operand_values.rs (1-1193)
5. flow_ownership.rs (1-1989)
6. flow_place.rs (1-1837)
7. flow_reference_places.rs (1-1108)
8. flow_retained_borrows.rs (1-1394)
9. flow_state_values.rs (1-3815)

(Paths below are relative to that `src/` directory. These files are submodules of `flow.rs`; `flow.rs:11742-11745` re-exports the mutation items.)

---

## 1. flow_mutation.rs (4762 lines) — read in full

### Purpose
Answers "which caller storage places may this statement or call write?" so flow can retire facts/observations at those places. Produces `CanonicalPlace` lists (and the richer `StorageRegion` enum) for assignments (`statement_storage_writes` :4573), calls (`call_mutated_places` :3828, `call_mutated_regions` :3849, `call_mutation_writes` :3870), exclusive access routes (`call_write_accesses` :3943), receiver-only writes (`call_receiver_write_accesses` :3975), and normalized write frames (`frame_storage_writes` :4619). Header comment (:1-6): when a write set is `None`, flow drops every active context. When no complete callee summary exists it falls back to the declared signature "ceiling" (`ceiling` mod :22).

### Shape (nested modules inside one file)
- `mod ceiling` (:22-689): `signature_ceiling_places` :66, `signature_ceiling_regions` :84, `linear_restoration_ceiling_regions` :110, `selected_signature_ceiling_regions` :148 (walks call-site parameters; exclusive-reference params -> `captured_argument_write_regions`; by-value params carrying exclusive loans -> `carried_exclusive_referents` :462; resident owners -> `resident_parameter_regions` :309 / `resident_owners_in_type` :388), `retained_loan_write_regions` :560, `carrier_retirement_may_write` :620, `loan_write_region` :638, `owner_path_overlaps` :651, `is_exclusive_reference` :434.
- `mod local_origins` (:690-1976): alias/origin projection. Sub-mod `coordinates` (:727-1242) with `OriginPrecision {Exact, MaySelect, Enclosing}` :741, `origin_place` :755, `source_place` :765, `write_source_region` :821, `output_write_region` :946, `result_write_region` :989, `structural_origin` :1021, `exact_origin_place` :1066, `alias_projection` :1086, `origin_type_reference` :1143, `sole_local_referent_origin` :1179 (+2 unit tests :1200-1241). Outside it: `close_storage_places_over_aliases` :1247, `close_storage_places_over_origins` :1273, `assignment_storage_places/regions` :1339/:1357, `place_requires_local_write_origin` :1484, `rebase_exact_local_place` :1512, `rebase_local_write_places/regions` :1559/:1713, `rebase_storage_region` :1575, `rebase_captured_storage_region` :1592, `rebase_region_owner` :1632, `rebase_descriptor_slot_regions` :1671, `descriptor_slot_is_inline` :1691, `rebase_write_regions` :1730, `rebase_reachable_storage` :1778 (worklist with `crossed` origin indices), `reference_result_candidate_places` :1882, `frame_write_place` :1914, `place_from_origin_path` :1928.
- `mod operand_coordinates` (:1978-2045): `call_operands_have_builtin_coordinates` :1988, recursive `place_operands_are_builtin` :2014 (depth cap 128 :2021, returns false at cap).
- `mod receiver` (:2046-2235): `call_receiver_is_mutable` :2061, `call_receiver_mutated_place` :2081, `canonical_receiver_place_for_call_site` :2111, `family_attached_data_symbol` :2177, `family_data_binding_parameter` :2204, `call_data_argument` :2219.
- `mod regions` (:2236-2415): `enum StorageRegion {Declared(place), UnresolvedContent{carrier}, ResidentContent{owner,formation,granted,resident}, NonresidentContent{owner,formation}}` :2252; methods `anchor` :2285, `into_declared` :2295, `declared_places` :2308 (returns `None` if any region is not Declared), `same_storage_space` :2314, `contains` :2326, `preserves_owner_qualification` :2344.
- `mod summary` (:2416-3778): `StateMutationSummaryCache` :2460 (a `OnceLock` of all state summaries), `StateMutationSummary` :2508 {state_symbol, complete, writes, held_writes}, `WriteDestination {Referent, DescriptorSlot}` :2522, `MutationWrite` :2528 {region, requires_exclusive_access, destination, checked_write_type}, `build_state_mutation_summaries` :2659 (rayon, `checking_thread_pool`), `propagate_state_mutation_summaries` :2725 (dirty-mask fixed point over callers), `instantiate_known_call_mutation_summary_places` :2535, `instantiate_known_call_receiver_summary_places` :2590, `collect_state_mutation_summary_places` :3202, `state_summary_exposes_place` :3170, `instantiate_call_relative_writes[_pre_resolved]` :3304/:3435, `AccessRouteReader` :3376, `captured_argument_write_regions` :3574, `rebase_projected_call_region` :3701, `write_reaches_caller` :3746, `storage_place_has_declared_identity` :3770.
- Top level (:3780-4762): `WritePlaceNamespace {AccessRoute, Storage}` :3798, `CallMutationWrite {region, source, checked_write_type}` :3811, `call_write_places` :4005, `exclusive_loan_places` :4112, `shared_call_storage_regions` :4190 (boundary-trait frames refined by exclusive referents), `boundary_frame_access_places` :4336, `storage_extends_frame_place` :4479, `statement_mutated_place` :4495, `atomic_once_storage_writes`/`atomic_once_write_effects` :4520/:4539, `normalize_write_only_range_place` :4651 (turns range-index segments of write-only reference roots into `FixedRange` when bounds are immutable integers), `descriptor_table_written_regions` :4721 (SGDT: exactly one operand, :4738).

### Inputs and outputs
Reads: `validation::CheckingInput` program tables, `BorrowFacts`/`BorrowCallFact`, `CallFrameResolver` (validation crate; local write origins, call write origins, signature frames), `find_call_site`. Outputs: `Option<Vec<CanonicalPlace>>`/`Option<Vec<StorageRegion>>`/`Vec<CallMutationWrite>`. `None` = unknown (caller must fully invalidate). Empty vec = complete, no writes (mutually distinct, stated :3821-3826).

### How it decides (keys)
- Complete summary vs ceiling: `state_mutation_summary` :2645 returns only entries with `complete`; `complete` requires direct writes known, concrete body signature (`state_has_concrete_body_signature` :3152), and `resolver.inferred_state_write_frame(..).is_complete()` (:2681-2689). Keyed on state symbol.
- Boundary target detection: `program.traits().iter().any(|d| d.is_boundary && trait_machine_signature_by_symbol(d, target).is_some())` :4065-4070 (identity via symbol).
- Fact-only calls: `validation::invocation_is_fact_only` :3879; builtin function symbol -> empty ceiling :159-165; byte predicate calls -> empty :3897-3901; GDT Load -> empty :4020-4025.
- Exposure of callee writes to caller: root symbol equals caller machine, or is a caller parameter (declared writes only if the param type is not primitive and not reference-free) :2855-2880, :3170-3200.
- Overlap of loan owner paths vs place segments :651-688 (absent/dynamic selector overlaps).
- Path-string use: `place_from_origin_path` :1928 splits a dotted `String` path on `.` and matches the first member by `parameter.name.as_str() == root_name` and local `name`; `frame_write_place` :1914 strips the `"[*]"` suffix text. That resolves write-frame report paths (strings) back to symbols by source spelling (names). Fields are resolved via `resolve_member_symbol_from_place` by member name :1956.

### Cases by shape
- `assignment_storage_regions` loop :1369-1393 walks Borrow/Member/Range-index only; an `Indexed` non-range target without builtin coordinates returns `None` (:1381-1390).
- `call_operands_have_builtin_coordinates`: argument/receiver expressions must be Borrow/Member/Indexed-range chains; plain `Indexed` needs `value_has_builtin_coordinates` :2039-2041; anything else `true`.
- `descriptor_table_written_regions` requires exactly one argument `let [operand] = arguments else None` :4738.
- `output_write_region`/`result_write_region` return None for image storage, recasts, representation :951-956, :995-1000.
- `write_source_region` ExtentView recasts: only `AccessLifecycleOperation::View` :848; resident recasts need a single matching `access_view_plans` row (`plans.next(), plans.next()` :904).
- `rebase_projected_call_region` for DescriptorSlot destination requires `Declared` region and reference-shell at both expected and actual :3723-3731.

### Same job elsewhere
- Several overlapping "rebase a place through local write origins" functions in this file: `rebase_local_write_regions` :1713, `rebase_captured_storage_region` :1592, `rebase_exact_local_place` :1512, `rebase_descriptor_slot_regions` :1671; they share `rebase_reachable_storage` :1778 except the exact one, which uses `exact_origin_place` :1066.
- `exclusive_loan_places` :4112 and `ceiling::selected_signature_ceiling_regions` :148 both compute "exclusive actual/parameter places of a call" (one from `argument_accesses` + `is_mutable` params, the other from reference-typed params via resolver).
- `StorageRegion::contains` :2326 and `storage_extends_frame_place` :4479 and `owner_path_overlaps` :651 are three place-prefix relations (read).
- Suspected duplicate outside unit: `canonical_place_segments_may_overlap` is defined in validation `storage_path_relations` (referenced :2394-2401 as `validation::storage_path_relations::...`) and also re-exported via `crate::flow` (:15); not read.

### Unused / one-caller code (grep -rwn --include=*.rs over omega-rust; "occurrences" includes definition and re-export lines)
- `signature_ceiling_places`: 7 occurrences, callers in `checks_contracts_prover.rs:4587`, `checks_multiplicity_claim_outcomes.rs:1573`, `flow_reference_places.rs:1086` (3 external call sites).
- `linear_restoration_ceiling_regions`: 4 occ; 1 external call `checks_multiplicity_linear_validation.rs:1838`.
- `call_mutated_regions`: 3 occ; 1 external call `checks_multiplicity_linear_validation.rs:1856`.
- `call_receiver_write_accesses`: 3 occ; 1 external call `checks_borrows_calls.rs:755`.
- `call_mutated_places`: 8 occ; external calls at `checks_contracts.rs:9943`, `checks_multiplicity_permission_events.rs:281`, `checks_ranges_facts.rs:426`, `execution_terminal_unit_calls.rs:4517` (4).
- `family_attached_data_symbol`: 2 occ (definition + its only use inside `family_data_binding_parameter` :2210), pub(crate) but never used elsewhere.
- `rebase_descriptor_slot_regions`: 2 occ (def :1671, use :1437), `pub(super)`.
- `atomic_once_write_effects`: 3 occ (def, use in `atomic_once_storage_writes` :4528, external `flow_expression.rs:11276`).
- `atomic_once_storage_writes`: 3 occ; used in this file :3215, :4584.
- `call_write_accesses` 8 occ, `statement_storage_writes` 12 occ / 8 files, `frame_storage_writes` 16 occ / 8 files, `rebase_exact_local_place` 24 occ / 8 files, `sole_local_referent_origin` 5 occ / 3 files, `place_from_origin_path` 7 occ / 3 files, `call_receiver_is_mutable` 6 occ / 2 files.
- Comment calls `CallFrameResolver` "the single owner of the complete-or-opaque call and cycle law" (:2420). Users of `CallFrameResolver` in this file are many (e.g. :187, :2664-2689); not counted outside.

### Stated limits
- "An opaque, unresolved, generic or dispatched callee ... retains the signature/authority ceiling" (:26-28).
- "Unknown capability content retires observations without authorizing any declared-storage replacement" (:33-34).
- "These are may-write coordinates, never evidence establishing a resident." (:2244).
- "A frozen window is an exact descriptor origin" (:1139).

### Fail-open risks
- `place_requires_local_write_origin` returns `false` when root is not a symbol or state/local lookup fails (:1491-1500) -> the place is then taken as declared storage, no rebasing through aliases (:1519-1527, :1741-1748). Wrong program: a write through a reference-typed local whose declaration lookup fails would be recorded as a write to the local itself rather than its referent. Status: open (not traced).
- `rebase_exact_local_place` :1512 returns the place unchanged when `place_requires_local_write_origin` is false; same dependency. open.
- `ceiling` `include_parameter` closure in `linear_restoration_ceiling_regions` :125-144: returns `true` (include) when paths unknown — fail-closed direction. Not a risk.
- `selected_signature_ceiling_regions` returns `Some(Vec::new())` for builtin functions :159-165 and for fact-only requirement citations :176-180 (stated: "observes its subjects without executing" :173). traced via `validation::is_fact_only_requirement_citation` name only; not read.
- `call_mutation_writes` returns empty set for any callee where `invocation_is_fact_only` :3879, and `byte_predicate_call` :3897; `activation_accessor_has_empty_write_frame`/`access_borrow_has_empty_write_frame` :4047-4051. Judgment lives in validation; not read. open.
- `statement_storage_writes` :4593: any non-assignment, non-atomic-local statement returns `Some(vec![])` (no writes). Calls are handled separately via `call_mutated_places`; stated by being a dedicated assignment path (no comment). open.
- `close_storage_places_over_aliases` `.or(Some(places))` at :4612: if alias closure fails (returns `None`, e.g. unresolved origin), `statement_storage_writes` falls back to the un-closed places instead of `None` (full invalidation), though its doc comment says "An unresolved origin requires full invalidation" (:4571). Wrong program: an assignment whose alias closure cannot be computed would retire facts only at the directly written place, not at aliasing locals. Status: open (did not trace what makes closure `None` vs the first `places` computation being already `Some`); partly mitigated because `assignment_storage_places` already rebases through origins (:4596-4603).
- `close_storage_places_over_origins` :1283-1289 skips origins flagged `is_image_storage` entirely (no alias added). open.
- `origin_path_overlaps`: Field with invalid place symbol overlaps everything :665 (conservative).
- `frame_storage_writes` / `call_write_accesses` use `.expect("access-route projection always retains the ownership fallback")` :3963 — panic if None; not a fail-open.

---

## 2. flow_observations.rs (1690 lines) — read in full

### Purpose
Rejoins a "case/tag observation" (a transition-guard membership test such as `x is Variant`) to the exact storage it read and the loans that formed its address, using the already-captured flow operand/observation graph rather than source spelling. Also decides whether a selected reference descriptor observed once agrees with a later consumer, and finds the selected payload copy a transition target consumes. Header (:1-6): a tag observation names the receiver evaluated, not a later read of its reference binding.

### Shape
- `mod captured_subject` (:26-641): `CapturedSubjectFacts` :53 {semantic, flow, borrow}; `CapturedCaseSubject` :69 {observed, source, receiver, loans}; `CapturedCaseSubjectFailure` :107 (4 variants, `reason()` :115); `captured_case_subject` :76 (Option), `captured_case_subject_result` :127 (Result), `captured_case_subject_in_state` :160, `captured_case_subject_in_state_result` :203, `completed_case_subject` :248 (the ~230-line gate list returning `Err(&'static str)` reason codes), `trace_captured_subject_refusal` :485 (eprintln diagnostics), `referent_place` :605.
- `mod current_values` (:642-782): `ScalarObservation` :661, `scalar_observation_at` :672, `current_scalar_observation` :749.
- `mod descriptors` (:783-1304): `descriptor_namespace` :804, `descriptor_source` :848, `same_referents` :903, `same_retention` :941 (nested `selected_profiles`, `alternatives_agree`, `profiles_agree` with `visited` cycle guard), `descriptor_capture` :1100, `DescriptorObservationRefusal` :1127 (13 variants), `observation_agrees` :1143, `rejoin_observation` :1238, `descriptor_context_agrees` :1257, `descriptor_observation_namespace` :1277.
- `mod dispatch_copies` (:1307-1478): `copied_payload_at` :1339, `completed_copy_read_place` :1407.
- Top level: `completed_dispatch_extraction` :1486, `CaseObservation` :1525, `case_observation` :1537, `PredicateCaseDependencies` :1634, `predicate_case_dependencies` :1639.

### Inputs / outputs
Reads `FlowControlFacts` (statements, value_observations, operands, operand_referents, case_accesses, referent_formations, operand_retentions, copied_values, outgoing...), `FlowContextFacts.constraint_refs`, `BorrowFacts` loans, plus `FlowBuildContext` for in-progress state. Output: `CapturedCaseSubject`, `CaseObservation`, `CanonicalPlace` namespaces, bool agreement, `PredicateCaseDependencies`.

### How it decides
- `completed_case_subject` :248-478 is a chain of identity gates keyed on handles/ordinals: exactly one `TransitionGuard` observation with `observation.expression == expression` (:272-281); receiver `expression`, `state_symbol`, `statement_index` equality (:286-290); a unique referent (`referent_place` :612 requires exactly one `[referent]`); reaching descriptor store via `at_snapshot`; formed statement strictly before observing statement in `admitted_execution_order` (:322-330); exactly one borrow-state row (:333); descriptor must be a whole binding (:344); live formation loan matched on owner_symbol, storage_owner, empty owner_path, kind, lineage != LifetimeDependency, statement_index, formation_expression (:385-401); observed vs completed-backing places agree by `canonical_places_agree` (:413); types equal or equal `applied_type_identity` (:422-439); binding unchanged via `operand_bindings_agree_in_state` for the source and each `Index` selector (:447-467).
- `descriptor_namespace` :804: needs readable reference shell, valid case_access matching operand expression/state/statement/constraints, exactly one source referent with no formation.
- `same_retention`/`profiles_agree` :941-1085: structural comparison of retention profiles (roots as sets, leaves pairwise by path, alternatives as set agreement). Missing/empty alternatives -> disagree (`!first.is_empty() && !second.is_empty()` :982-983).
- `case_observation` :1537: unique value observation for expression at statement; 0 referents -> None, 1 referent -> place, many -> requires `descriptor_capture` whose `case_access.evaluation`/`constraints` equal the observation's (:1589-1593); every place must pass `super::stable_case_subject` (:1604-1608); readable observed vs source type equal or equal applied identity (:1612-1619).
- `copied_payload_at` :1339: only `FlowPermissionEvaluation::TransitionTarget(target)` (else None :1348); matches outgoing rows by `statement_index` + `transition_target`; payload binding via `DispatchExtraction::reads_payload_binding` (validation); prefix equality of segments; unique match required (:1398-1399).
- `predicate_case_dependencies` :1639 collects case-membership `Binary` nodes via `visit_pre_order`, using `applied_case_membership_variant` (validation).
- Trace gating `::validation::trace_flags::check_sites()` :185, :1247.

### Cases by shape
- `descriptor_context_agrees`/`descriptor_observation_namespace`: `descriptor_observation_namespace` returns `None` if two contexts' rejoined namespaces differ (:1294-1299).
- `completed_copy_read_place` :1472-1476: 0 or 1 matching copies OK, more than one -> None.
- `captured_case_subject_in_state_result` requires a binary membership node (:215-217).
- `completed_case_subject` forwarded vs non-forwarded descriptor: exactly one `places` for forwarded (:362) else uses `formed.backing` with `BorrowAccessKind::Read`.

### Same job elsewhere
- `descriptor_source`/`same_referents` (:848-939) and `completed_case_subject` (:248) both compare referent paths between operands; `canonical_places_agree` is from `flow.rs` (imported :34).
- The applied-type-identity equality block is written three times: :422-439, :1174-1190, :1612-1619 (read). Same job with the same idiom `applied_type_identity(program.typed_program(), x, machine, &[])`.
- `referent_place` :605 vs `case_observation`'s `canonical` closure :1572 and `scalar_observation_at` :710-720: three inline "referent -> CanonicalPlace(root, segments span)" builders (read).
- `scalar_observation_at` :689-701 and `case_observation` :1545-1558 repeat the same "find the unique value observation for an expression in this state's appended statements" filter.

### Unused / one-caller code (grep -rwn)
- `completed_dispatch_extraction`: 3 occ; 1 external call `borrow.rs:411`.
- `captured_case_subject_result`: 4 occ; 1 external call `checks_multiplicity_linear_validation.rs:1015`.
- `captured_case_subject_in_state`: 4 occ / 2 files (used by flow internals); `captured_case_subject` 6 occ / 4 files.
- `descriptor_context_agrees`: 5 occ; external callers `checks_contracts_prover.rs:1003`, `flow_operand_contexts.rs:365`.
- `descriptor_observation_namespace`: 6 occ; callers `checks_contracts_prover.rs:972`, `flow_expression.rs:7069`.
- `copied_payload_at`: 7 occ; callers `checks_borrows.rs:4850`, `execution_scalar_computations.rs:3385, :3494, :6826` (4 callers).
- `predicate_case_dependencies`: 4 occ / 2 files; `case_observation` 10 occ / 4 files; `CapturedCaseSubjectFailure` 9 occ / 1 file (only this file).
- Comment (:1274-1275) says predicates and payload-case checks "share this one rejoin": callers of `descriptor_observation_namespace` outside this file are 2 (`checks_contracts_prover.rs:972`, `flow_expression.rs:7069`), `descriptor_context_agrees` has 2 outside.

### Stated limits
- "The Option query above and its in-state caller retain their original acceptance behavior." (:104-105)
- "A nonzero but unavailable occurrence is unknown, not a declaration fallback." (:670)
- "This grants no address, extent, loan or lifetime authority." (:1272-1273)

### Fail-open risks
- `descriptor_context_agrees` :1257-1269: `.all(...)` over `descriptor_context_observations.for_context(context)`; with no rows for the context it returns `true`. Wrong input: a context with zero descriptor observation rows is treated as agreeing. Status: open (the empty case is plausibly intended - a context with no descriptor dependence - not traced).
- `completed_copy_read_place` :1473: no matching copy returns the place unchanged ("An unmatched place keeps its ordinary identity" :1403-1404) - stated.
- `current_scalar_observation` :763: when `read` is `None` (declared/formal algebra) skips `stable_case_subject` and prefix-agreement checks (stated :667-671 "A zero observation belongs to declared/formal algebra"). Stated.
- Others: gates return `None`/`Err` on missing data (fail closed). None seen.

---

## 3. flow_operand_contexts.rs (1136 lines) — read in full

### Purpose
Decides which fact contexts (assumed facts) are valid for one call/operator operand, and answers "did this operand's observed contents/binding survive intervening writes?" using the recorded write prefixes. Header (:1-5): detached copies retain completion facts; exact views read their referents at invocation; "Operators and ordinary calls share this law".

### Shape
- `mod selections` (:7-185): `OperandContextSelections` :30 (BTreeMap cache keyed by `OperandSelectionKey` :45 {operand, stable_copy, invocation, state, statement_index}; `scratch` slot for invalid handles), `OperandContextSelection` :57 {contexts, lazily-built `cases`/`memberships` OnceCells}; methods `has_membership` :69, `cases_are_selected` :95, `path_is_inactive` :106, `for_operand` :135.
- `mod write_intervals` (:186-300): `ContentObservationPoint` :202, `contents_observation_survives` :221, `contents_writes_between` :241 (chronological write interval across statements in `admitted_execution_order`).
- Top-level: `captured_call_operands` :316, `operand_contexts` :349, `invocation_contexts_at_referents` :403, `operand_binds_live_view` :495, `operand_binds_stable_copy` :523, `operand_entry_origin_is_stable` :557, `operand_entry_binding_origin_is_stable` :576, `entry_origin_is_stable` :591, `case_overwrite` :605, re-export of `binding_overwrite`/`contents_overlap` from validation :618, `binding_origin_is_stable` :625, `SourceWriteRelation` fn-pointer type :635, `entry_origin_is_stable_when` :641, `operand_observations_agree` :670, `operand_bindings_agree` :692, `operand_sources_agree[_in_state]` :709/:758, `operand_observation_before_statement_agrees` :833, `operand_binding_before_statement_agrees` :852, `call_return_before_statement_agrees` :875, `observation_reaches_statement[_in_state]` :947/:971, `observation_prefixes_agree` :1050, `operand_contents_writes_between` :1070, `write_prefix_extension` :1091, `prefixes_agree` :1113.

### Inputs / outputs
Reads `FlowControlFacts` (operands, operand_origin_writes, statements, copied_values, captured_reference_leaves), `FlowContextFacts`, `FactPlan` (contexts/facts/places), the typed type-reference table. Outputs: `Vec<FactContextHandle>` or `bool`.

### How it decides
- `operand_contexts` :349-401: start from operand's captured constraint contexts; if the formal `operand_binds_stable_copy` return them unchanged (:361-363); otherwise intersect with descriptor-agreeing contexts (:367) and with the invocation's contexts by `(arena_index, generation)` identity (:374-378); for operands with referents or `Copied` completion add invocation contexts whose facts' places overlap the operand's recorded referents (:379-398, via `invocation_contexts_at_referents` :403, using `canonical_place_segments_may_overlap`).
- `operand_binds_stable_copy` :523-550: keyed on type shape: `Constrained` unwrapped; `Named` -> primitive or `has_contents(StableObservation)`; `Generic`/`FixedArray` -> `has_contents(StableObservation)`; anything else (Reference, Slice, ...) false.
- `operand_binds_live_view` :495: type shape: `Reference` or `Slice` (after unwrapping `Constrained`) -> true; else false. Invalid type -> false.
- Stability judgments key on storage overlap of recorded write rows with `place` via `contents_overlap` / `binding_overwrite` from validation (:618), with `root` equality first (`place.root != write.root`).
- `operand_sources_agree_in_state` :758: same `FlowOperandFact` pointer -> true (:771); unknown writes -> false; different states -> false; different statements -> earlier/later by `admitted_execution_order` position; same statement -> `prefixes_agree`.
- `prefixes_agree` :1113-1135: one write-span must be a row-equal prefix of the other (`write_prefix_extension` :1091 compares `root` and segment slices of each row), then extra writes must not overlap `place`.
- `OperandSelectionKey` caches by handle `(arena_index, generation)` identity.

### Cases by shape
- `call_return_before_statement_agrees` :892-944: `CallSite::Expression` -> every operand for that expression must agree; `CallSite::Statement` -> only later admitted statements' `completed_content_writes` are checked; `CallSite::TransitionNamed` -> `false` (:943).
- `captured_call_operands` :316: receiver occupies one evaluation position only for `CallSite::Expression` with valid `call.receiver` (:329-335).
- `case_overwrite` :605-616: descendant-write exemption applies only when all segments have resolved identity.
- `operand_binds_stable_copy` handles `Named`/`Generic`/`FixedArray` only ("A copied scalar is the narrow case; a by-value record, sum, or fixed array ..." :513-515).

### Same job elsewhere
- `entry_origin_is_stable_when` :641 (write-row overlap against a place) and `contents_observation_survives` :221-239 and `prefixes_agree` :1129-1135 each repeat the same "all writes either differ in root or don't change place" loop with `write_changes` (read; 3 copies in this file).
- `contents_writes_between` :241 and `observation_reaches_statement_in_state` :971 both walk `admitted_execution_order` between two statement indices (read).
- Suspected duplicate: `descriptors::descriptor_context_agrees` (flow_observations.rs:1257) is the only context filter called here (:365).

### Unused / one-caller code (grep -rwn --include=*.rs, occurrences incl. defs/re-exports)
- `case_overwrite`: 3 occ / 2 files; 1 external call `flow.rs:6954` (import `flow.rs:6893`).
- `operand_entry_binding_origin_is_stable`: 4 occ / 4 files.
- `operand_binding_before_statement_agrees`: 3 occ / 3 files (def, re-export, 1 use).
- `observation_prefixes_agree`: 3 occ / 2 files (def + 1 use in flow_observations.rs:767 + import).
- `operand_observation_before_statement_agrees` 4 occ; `call_return_before_statement_agrees` 4 occ; `contents_observation_survives` 4 occ / 2 files; `operand_contents_writes_between` 5 occ / 4 files; `operand_bindings_agree` 6 occ / 6 files; `OperandContextSelections` 6 occ / 3 files.
- Comment :2-4 says operators and ordinary calls "share this law"; `operand_binds_live_view` 48 occ / 16 files and `operand_binds_stable_copy` 15 occ / 8 files, so those two are widely shared.

### Stated limits
- "anything the contents classifier cannot prove stable, or whose referent could not be resolved exactly, keeps the capture ∩ invocation intersection." (:520-521)
- "A dense operand-by-type table would reserve the Cartesian product" (:36) — explains BTreeMap (note AGENTS.md prefers dense tables for dense IDs; this one is keyed by sparse pairs, stated).

### Fail-open risks
- `operand_sources_agree_in_state` :771: pointer-equal operands return `true` without checking unknown writes. Wrong program: none - same operand compared to itself. Traced (trivially).
- `OperandContextSelections::for_operand` returns `None` if operand does not belong to this flow (:143) - callers decide; not fail-open by itself.
- `operand_contexts`: when `operand_binds_stable_copy` is true, contexts are returned without descriptor/invocation intersection (:361-363). Stated by the doc (:511-522): detached stable copy facts "describe the invocation value outright". Stated.
- `OperandContextSelection::cases_are_selected` :95-104: a subject with no `Case` segment returns `true` immediately (no cases needed). Traced: nothing to select.
- `path_is_inactive` returns `false` when `cases()` is `None` (cases evidence unavailable) :111-114 — i.e. a case path is treated as NOT inactive when evidence cannot be prepared. Wrong program: a path through an inactive variant whose evidence failed to prepare is treated as active (more facts kept/checked, which usually errs conservative for obligations but could keep facts on an impossible path). Status: open.

---

## 4. flow_operand_values.rs (1193 lines) — read in full

### Purpose
Projects a call's/constructor's completed operand tree (`FlowOperandFact` rows in `FlowControlFacts`) onto the child that supplied a given field/element path, and from there to the storage places and fact contexts that operand denotes. Header (:1-6): destination paths identify fields independently of authored evaluation order; each child keeps its own completion context and type.

### Shape
- `mod captured_operand` (:8-88): `CapturedOperand<'capture>` :20 {row, handle, owner}; `recorded` :31 (valid handle in its owner or None), `unrecorded` :42 (scratch borrow, invalid handle), `recorded_in` :58, `belongs_to` :71; `Deref` to the row.
- `mod reference_places` (:89-307): `captured_reference_places_with_formation` :107, `captured_operand_reference_places_with_formation` :148, recursive `collect_operand_reference_places` :168 (cycle guard `ancestors` :136), `collect_leaf_places` :281.
- `mod result_memberships` (:308-458): `CapturedResultMemberships` :333 (BTreeMap cache keyed `(arena_index, generation)`), `contains` :349, `prepare_promises` :406 (collects `FactOrigin::CallEnsures` domain-membership facts rooted at the operand expression, with `StorageDependency` dependencies).
- Top level: `CapturedValue` :485 {observation, observation_type, observation_projection, operand, type_reference, projection, producer_epoch, choices}; `CapturedValueChoice` :510; `CapturedValuePlace` :549 {storage, projected}; `captured_lifetime_roots` :562; `captured_observation_roots` :589; `captured_value_places` :604; `captured_observation_places` :622; `captured_places` :641; `captured_operand_values` :740 (recursive; Copied / Selected / literal-projection branches); `primitive_read_source` :983; `observe_captured_values` :1005; `reaching_store_values` :1039; `captured_operand_at_path` :1130; `captured_operand_handle_at_path` :1147; `projected_capture` :1159. Re-exports from `validation::completed_storage` :475-478 (`captured_operand_child_at_path`, `captured_retention_profiles_at_path`, `finite_projection_paths`, `operand_child_type`, `project_captured_view_place`).

### Inputs / outputs
Reads `FlowControlFacts` arenas (operands, copied_values, captured_reference_leaves, operand_alternatives, operand_referents, operand_child_paths), `FactPlan` semantic facts, `stored_values::at_snapshot`. Produces `Vec<CapturedValue>`, `Vec<CapturedValuePlace>`, `Vec<CapturedReferencePlace>`, `Vec<PlaceRoot>`.

### How it decides
- Completion kind drives the branch: `FlowOperandCompletion::{Selected, Copied, Constructed, ...}` (:191, :223, :753, :804, :902). A `Selected` operand needs every alternative's `source_arm` valid and a valid `value` (:208, :811) or the whole query is `None`.
- Live-view vs detached is decided from the type shape through `operand_binds_live_view`/`operand_binds_stable_copy` (flow_operand_contexts.rs:495/:523), e.g. :192, :235, :683, :700, :1053-1056.
- `collect_leaf_places` :281-306: leaf selected by `projection.starts_with(path)`; two matching leaves -> `None` (:290).
- `result_memberships::contains` :349-403: promise matched on `domain` symbol, `instance`, and `promise.path == value.projection.remaining` (exact path equality, :377-379), non-empty dependencies, and no intervening write overlapping a dependency place (:381-401).
- `reaching_store_values` :1039: requires one source place (`[source]` :1061), a `Symbol` root (:1065), and the selected store's projected type to be a live view (descriptor case) or a stable copy / same normalized type identity (:1096-1113).
- `primitive_read_source` :983: same primitive type on selected and captured types, source reference readable, referee primitive equal.
- `captured_places` :641: Copied -> leaf referents else non-live-view storage rooted at `PlaceRoot::Expression(source.expression)`; otherwise referents of a live view or `place_at(...)` for detached.

### Cases by shape
- Projection containing `PlaceSegment::Index` is expanded through `finite_projection_paths` (:177-187); no expansion means `None` upstream.
- `captured_operand_values` literal branch: `LiteralValueSource::{Expression, OmittedZero}` only (:854-869).
- `installed_child` special case for `Constructed` completion with non-empty destination and non-view type (:901-905).

### Same job elsewhere
- "Project a captured operand to the place(s) it denotes": `captured_places` :641 and `collect_operand_reference_places` :168 both branch on `Copied`/live-view/referents; one returns places of values, the other reference referents (read).
- `captured_operand_child_at_path` is in validation (`validation::completed_storage`), re-exported here :476; flow_observations.rs:313 uses `captured_operand_handle_at_path` for the same walk (read).
- `CapturedResultMemberships` (BTreeMap keyed by `(arena_index, generation)`) and `OperandContextSelections` (flow_operand_contexts.rs:39) are two per-invocation caches of the same shape.

### Unused / one-caller code (grep -rwn --include=*.rs; occ = occurrences incl. defs/re-exports)
- `captured_observation_roots`: 3 occ; 1 caller `checks_multiplicity.rs:10080`.
- `captured_operand_at_path`: 5 occ; callers `checks.rs:1311`, `checks_contracts_writes.rs:1467`, `flow_transfers.rs:566`.
- `captured_reference_places_with_formation`: 5 occ / 3 files (callers include flow_observations.rs:354).
- `captured_operand_reference_places_with_formation`: 9 occ / 4 files (callers `flow.rs:1668`, `flow_expression.rs:3110, :7116`, `flow_retained_borrows.rs:329, :466`).
- `observe_captured_values` 5 occ / 5 files; `captured_operand_handle_at_path` 6 occ / 4 files; `captured_lifetime_roots` 12 occ / 6 files; `captured_value_places` 23 occ / 12 files; `captured_observation_places` 19 occ / 10 files; `CapturedResultMemberships` 7 occ / 4 files; `projected_capture` 5 occ / 1 file.
- `CapturedValue` 141 occ / 10 files.

### Stated limits
- "Unknown producers and unknown saved descriptor inventories decline without substituting a declaration initializer or a newer source descriptor." (:95-96)
- "Unknown writes stop provenance, leaving the current storage observation and its own contexts intact." (:1036-1037)
- "No initializer or callee body is consulted to replace a missing capture." (:547-548)

### Fail-open risks
- `collect_operand_reference_places` :188-190: `literal_value_path_is_inactive(...)` true -> returns `Some(())` with no places added (a "constructor with a provably different enclosing case contributes no address", stated :92-94; the doc states the live-case check still proves payload access). Stated.
- `reaching_store_values` returning `None` makes `captured_operand_values` fall back to the current observation `values.push(value)` (:971-973) - falls back to unrefined observation namespace (stated by :1036-1037). Stated.
- `captured_operand_values` Copied branch :758-782: when a selected path lands inside a captured reference leaf, returns a value tied to the operand itself without descending to the source (referent facts come from the leaf). Traced as designed; no evidence of a wrong program reaching it.
- None else seen.

---

## 5. flow_ownership.rs (1989 lines) — read in full

### Purpose
Walks one state's admitted statements and its borrow calls in execution order and records which caller storage places a statement/call consumes ("move events", `FlowStorageTransferFact`), distinguishing an exclusive reference's descriptor move from consumption of its referent. Output feeds multiplicity checks and terminal cleanup. Header (:1-11): "A place's declared type names storage; the selected operand type names the value transferred from it."

### Shape
- `pub(crate) mod calls` (:20-307): `append_call_ownership_events` :36, `affine_call_result_type` :167, `owned_call_operand_places` :200, `owned_operand_transfer_place` :235, `transfers_for_source` :293.
- `mod events` (:308-419): re-exports `FlowStorageTransferFact as DiscoveredMoveEvent`, `FlowStorageTransferSource as FlowOwnershipEventSource` :311; `DirectMoveEventSink` :316 {segments arena, operators, machine, state, events, proof_only, evaluation, source_arm}; `append_move_event_for_place` :379; `call_permission_evaluation` :393 (Transition statement -> `TransitionTarget(target)` / `TransitionGuard` / `Unknown`; other statement -> `Statement`; no statement -> `Unknown`).
- `mod moves` (:420-1455): sub-mod `observations` :436 (`append` :458, `append_indexed_operands` :517, `selected_operator` :599, `detached_borrowed_copy_admitted` :645, `call_target_is_service_signature` :720, `reference_access` :736); `consumed_value_type` :759; `append_move_events_for_expression` :779 (the main recursive walk); `selected_meaning_borrows_operands` :1029; `append_move_events_for_call_arguments` :1066; `expression_call_is_owned_by_call_flow` :1137; `append_move_events_for_operator_statement_call` :1186; `OperatorOperandOwnership`/`OperatorCallOwnershipPolicy` :1242/:1256; `operator_call_ownership_policy` :1300; `receiver_expression_static_path_segments` :1341; `initializer_produces_owned_value` :1383; `reference_slot_is_borrowed` :1418.
- `mod result_storage` (:1456-1589): `is_private_result_place` :1467.
- `mod type_references` (:1590-1798): `OperatorResultOwnership {BorrowedView, OwnedValue, NoTransfer}` :1612; `classify_operator_result_ownership` :1641; `type_requires_ownership` :1672 (valid && `type_multiplicity != Unrestricted`); `stored_value_requires_transfer` :1686 (`stored_value_multiplicity != Unrestricted`); `expression_is_place_like` :1694; `intrinsic_enum_equality` :1731.
- Entry points: `append_statement_ownership_events` :1812, `discover_state_move_events` :1945.

### Inputs / outputs
Reads typed program tables, `BorrowFacts` (calls span for the state), `CheckedOperatorFacts` (resolved operator uses), `FlowControlFacts.ownership.storage_transfers`. Writes `Vec<DiscoveredMoveEvent>` {source, evaluation, expression, value_type, source_arm, root, segments} via the sink (segments inserted into a `PagedArenaSet`).

### How it decides
- Whether a parameter transfers: `type_requires_ownership(parameter.type_reference)` = multiplicity != Unrestricted (:91), minus "proof call" affine params without a linear obligation (:76-99). Proof call = requirement citation, or any affine declared parameter and the target state's machine is a proof machine (`sink.proof_only(program).is_proof_machine`) (:76-89). Keyed on type multiplicity and machine identity.
- Move vs observe for a place-like expression: `expression_is_place_like` (node kind, :1694-1725) and `stored_value_requires_transfer(value_type)` or no value type (`unwrap_or(true)` :867); then observe if value type is a reference whose declared carrier does not match `expected_type` or the slot is borrowed (:875-889), or `detached_borrowed_copy_admitted` (:891-897); else a move event at `ownership_place_from_expression_in_state`.
- `detached_borrowed_copy_admitted` :645: place root must be a symbol; walks every proper prefix collecting reference access (`has_reference`, `has_exclusive`, `has_write_only`); write-only link rejects; shared-only chain with an index segment needs `Unrestricted` multiplicity + `StableObservation` contents; otherwise needs `source` to be a `Call` whose target is a `boundary` trait machine signature and `SeamMarshal` contents (:700-704). `call_target_is_service_signature` :720 matches `definition.is_boundary && trait_machine_signature_by_symbol(...)` over all traits.
- `selected_operator` :599: linear find over `operators.resolved_uses()` keyed on `expression`, `CheckedOperatorOccurrence::Expression`, and origin `StateStatement{state_symbol, statement_index}`.
- Static-path receiver: `receiver_expression_static_path_segments` :1341 treats a `Name` path as static when its first valid path symbol has no value type; then returns the *textual* name-path members `&str` (:1360-1366). `resolve_operator_for_call(..., call.target.as_str(), ...)` (:1083-1090, :1398-1405) resolves operators using that spelled target string and path segments (source spelling).
- `is_private_result_place` :1467: place rooted at an Expression that is a call whose return type is a single data definition (`count() != 1` -> false :1493-1496) or fixed array, then every segment must project through declared Case/Field members.
- `call_permission_evaluation` keyed on statement kind and `transition_call_target`.

### Cases by shape
- `append_statement_ownership_events` :1819-1937: exhaustive match over `StatementNode` (`Assembly`/`AssemblyFact` do nothing :1820, Assignment, LocalData, Transition, Call, Expression).
- `append_move_events_for_expression` final match :927-1017 lists expression kinds; `Name | Member | Indexed | TypeExpression | spelled_literal_expression_pattern!()` do nothing (:1012-1016).
- `intrinsic_enum_equality` :1731: only `==`/`!=` over a payload-free nominal sum with identical nominal symbol on both sides (:1779-1789).
- `operator_call_ownership_policy` :1300: comments "Three shapes exist" (:1294-1298): declared `self`, method form on a value receiver (requires `positional.len() == argument_count + 1` :1319), static path positional.
- `append_indexed_operands` :547-562 supports operand shapes `[collection, range.start, range.end]` or `[collection, index, invalid]`.
- `affine_call_result_type` :186-193: Named/Generic/FixedArray result that is Affine and has no linear obligation.

### Same job elsewhere
- `selected_meaning_borrows_operands` (:1029) and `intrinsic_enum_equality` (:1731) both decide "binary operator observes both operands"; both are used in the same `if` at :816-817.
- `moves::observations::append` (:458) vs `append_move_events_for_expression` (:779): the former is the observation walk, the latter the consumption walk; both recurse on Indexed/Borrow/Cast.
- `call_target_is_service_signature` :720-733 repeats the `traits().iter().any(|d| d.is_boundary && trait_machine_signature_by_symbol(d, target).is_some())` test that flow_mutation.rs:4065-4070 also writes (read both).
- `affine_call_result_type` :167 and `append_move_events_for_call_arguments` :1066 both resolve call targets (`resolve_state_call_target`, `call_target_return_type`); flow_mutation.rs resolves via `find_call_site` instead.
- Suspected: `classify_operator_result_ownership` doc (:1635-1639) says it "mirrors `type_requires_ownership`"; both are in this file.

### Unused / one-caller code (grep -rwn --include=*.rs)
- `append_statement_ownership_events`: 2 occ (def + one use :1965) — `pub(super)`.
- `append_call_ownership_events` 4 occ / 1 file; `initializer_produces_owned_value` 4 occ / 1 file; `classify_operator_result_ownership` 5 occ / 1 file (used once besides def); `is_private_result_place` 2 occ (def + use :224); `stored_value_requires_transfer` 3 occ / 1 file.
- `owned_call_operand_places`: 4 occ; 1 external caller `checks_borrows_calls.rs:256`.
- `owned_operand_transfer_place`: 4 occ; 1 external caller `checks_multiplicity.rs:6093`.
- `discover_state_move_events`: 4 occ; 1 external caller `checks_multiplicity_permission_events.rs:835`.
- `call_permission_evaluation`: 21 occ / 8 files (about 14 external call sites in checks_borrows.rs, checks_content.rs, checks_multiplicity*.rs, execution_terminal_unit_calls.rs).
- `type_requires_ownership` is `pub(in crate::flow::ownership)`, 8 occ / 1 file.

### Stated limits
- "Unknown (no resolved operator) preserves the legacy behavior" (:1237-1238).
- "a future copy/drop policy" / "A later policy lookup ... plugs in here" (:1606-1609) — stated seam, no policy present.
- "Unknown geometry retains the candidate for ordinary ownership validation" (:1415-1416).
- "so this only handles operator/boundary targets, the one statement-call class with no borrow-call fact" (:1178-1179).

### Fail-open risks
- `append_call_ownership_events` :67-69: when `call_target_parameters` is `None` (unresolved callee) it returns without emitting any move events. Wrong program: a call whose target parameters cannot be resolved that consumes an owned/affine argument records no move. Status: open (not traced; possibly earlier phases reject unresolved targets).
- Same function :52-64: a saved-dispatch call whose canonical statement/expression differs from this occurrence emits nothing (stated: it is owned by the canonical call). Stated by the check itself; counterpart traced only by name.
- `append_move_events_for_call_arguments` :1074-1078: returns (no events) when `call_target_parameters` is Some or `expression_call_is_owned_by_call_flow`; that is the intended hand-off to call flow (comment :1127-1135). Stated.
- `reference_slot_is_borrowed` :1418: returns `false` (i.e. keep the move candidate) on missing type info — fail-closed ("Only a positively known borrowed boundary suppresses a descriptor move" :1414).
- `append_move_events_for_expression` :862-868: `value_type.map(..).unwrap_or(true)` — unknown value type is treated as requiring transfer (fail-closed).
- `append_move_events_for_expression` :869-874: if `ownership_place_from_expression_in_state` returns `None` for a place-like expression, nothing is recorded (:909-911). Wrong program: a place-like expression whose ownership place cannot be formed is neither moved nor observed. Status: open.
- `is_private_result_place` returns false on any lookup failure, so such a place stays in the moved roster (fail-closed).
- `unreachable!()` at :820 and :1561 (panic sites, not accepts).

---

## 6. flow_place.rs (1837 lines) — read in full

### Purpose
Flow-dependent place judgments: narrowing runtime index selectors to fixed indices when the live facts pin them, deciding from live tag (case) observations whether a case-payload place is active/inactive/selected, and decomposing constructor literals into per-field/element projections. Header (:1-5): exact typed-program projection lives in validation; "these children narrow live selectors, compare paths and check case access".

### Shape
- `mod canonicalization` (:7-202): `narrow_pinned_index_segments` :26, `narrow_pinned_index_segments_with_values` :52, `canonical_place_from_symbol` :90, re-export `canonical_place_from_semantic_place` (validation) :97, `projected_leaf_place` :105, `source_independent_place` :161, `reference_source_place` :186.
- `mod case_access` (:203-1265): inner `mod observations` (:217-485) with `CaseObservation` :232, `NormalizedPlace` :252, `collect` :262, `normalize_place` :358, `Observer::predicate` :407; `CaseEvidence` :488 {observations sorted by place identity, `indexed_observations`, optional `CurrentScalarValues`}; `prepare` :511, `case_truth` :561, `cases_are_selected[_after]` :588/:592, `path_is_inactive` :616, `for_each_inactive_owned_case` :641, `has_value` :734, `locate_normalized_subject` :792, `has_value_for_subject` :822, `has_value_for_call_projection` :871, `case_evidence_proves` :965, `dispatch_copy_carries_case` :1012, `places_name_same_value` :1072, `place_root_is_immutable` :1094; free fns `place_cases_are_selected[_after]` :1120/:1146, `place_case_path_is_inactive` :1181, `place_case_has_value` :1209, `stable_subject` :1231.
- `mod coverage` :1266 and `mod domain_membership` :1272: pure re-exports (`segments_cover_subject`, `MembershipEvidence` from `validation::membership_evidence`).
- `mod literal_projection` (:1278-1799): `literal_argument_access_places` :1297, `zero_values::selected_type` :1333, `LiteralValueSource {Expression, OmittedZero}` :1383, `LiteralValueProjection` :1397, `LiteralExpressionProjection` :1417, `literal_expression_projections` :1423, `ProjectionMode {Access, SelectedValue, ConstructedValues}` :1443, `literal_value_path_is_inactive` :1455, `literal_value_projections` :1519, `literal_projections` :1539 (worklist over struct/array literals).
- Re-exports :1801-1824 of storage path relations from validation; `fixed_selection_segment` :1832.

### Inputs / outputs
Reads `FactPlan` contexts/facts (`BooleanValue`, `BooleanExpression`, `ContractBooleanExpression`, `CaseValue` payloads :274-280), expression table, scalar facts via `crate::values::scalar_value_at_place` and `CurrentScalarValues`. Produces `CanonicalPlace`s, bool judgments, `LiteralValueProjection` lists.

### How it decides
- Index narrowing `narrow_pinned_index_segments_with_values` :52-88: an `Index` segment becomes `FixedIndex` if the selector expression's `expression_interval` has `low == high` (:63-68), else if `scalar_at_place(selector)` is an `Integer` (:77) fitting `u64`/`usize`; otherwise stays `Index`.
- Case observation collection :262-350: from facts of those four payload kinds in the given contexts; boolean predicates walk `!`, `&&` when value true, `||` when value false, `== true` either side when `has_builtin_decomposed_guard_meaning`, and `applied_case_membership_variant` binary nodes (:407-483). `CaseValue` requires a `Variant` symbol and a convertible place (:301-314).
- `has_value_for_subject` :822-869: observation rows matched by exact normalized place identity (`*row.place == *subject`, partition-point on `compare_identity`) plus indexed rows whose `canonical_places_agree` or `same_scalar_projection` (:843-856); variant parent equality (`symbols.get(row.variant).parent == ...(case).parent` :859); selected row: `(row.variant == case) == required`; excluded row: `row.variant == case && !required`.
- `case_evidence_proves` :965-1004: direct evidence, or when `required` all other declared variants must be excluded (`has_case_evidence(alt, false)`) and at least one exists.
- `stable_subject` :1231-1264: root must be valid Symbol/Expression or an `InvocationReferent` equal to `entry_origins::invocation_referent`; all segments Field/Case with valid symbols, `FixedIndex`, or an `Index` whose expression has an immutable denotation (or current scalar values say so). Keyed on symbol validity and expression identity.
- `dispatch_copy_carries_case` :1012-1070: a local copy `let` that is immutable with an ordinary initializer, whose initializer place names the same value (`places_name_same_value`) and whose root is an immutable parameter/local (`place_root_is_immutable`) carries the case back to the source.
- `literal_value_path_is_inactive` :1455-1517: for each `Case` segment, all literal projections of the expression at the prefix must be struct literals / variant names whose actual variant differs from `variant` but has the same parent. Keyed on resolved symbols.
- `projected_leaf_place` :105: receiver chain acyclic (visited list), `Name` head symbols consistent, every `Member` re-resolved by `effective_member_symbol` equals the retained `member_symbol` (:140-146), no unresolved segments.
- `literal_projections` :1539-1798: `ExpressionNode::{StructLiteral, ArrayLiteral}` expanded, everything else becomes a single projection with the remaining path (:1788-1795).

### Cases by shape
- `source_independent_place` :161-179: statically known Field/Case/FixedIndex segments only; `FixedRange`, `OpaqueRange`, `Index` -> None (:171-173).
- `literal_projections` array case :1737-1787: requires `FixedArray` with `FixedArrayLength::Literal(length)` and `elements.len() == length` (:1741-1748); path segment shapes `None`, `FixedIndex`, `Index`, `FixedRange` accepted; anything else `return None` (:1785). Struct literal: variant-bearing sums require `case_symbol` else `None` (:1616-1621).
- `zero_values::selected_type` :1333: only Field (non-payload) and in-range literal `FixedIndex` segments; any reference carrier -> None.
- `fixed_selection_segment` :1832: Field, Case, FixedIndex only ("A runtime `Index`, a window, and an opaque selector stay variable" :1829).
- `intrinsic`-style: `Observer::predicate` depth limit via `expression_at_depth(expression, depth)`.

### Same job elsewhere
- `normalize_place` (:358-389) re-narrows `Index` to `FixedIndex` using both `narrow_pinned_index_segments` and `constant_integer_value`; `local_origins::owner_path_overlaps` (flow_mutation.rs:676-681) uses the same `constant_integer_value` idea. Comment (:19-24) says `transfers::write_target` applies the same narrowing (suspected; `flow_transfers.rs` not in this unit — grep `narrow_pinned_index_segments`: 11 occ in 6 files).
- `stable_subject` (:1231) vs `flow_observations.rs` `stable_case_subject` calls (flow_observations.rs:766, :1605): this is the same function, re-exported as `stable_case_subject` :1813.
- `places_name_same_value` + `place_root_is_immutable` (:1072-1117) vs. `local_origins::sole_local_referent_origin`: different questions, not a duplicate.
- `has_value_for_call_projection` :871 duplicates the observation scan of `has_value_for_subject` :822 with `matches_call_argument_projection` (read).

### Unused / one-caller code (grep -rwn --include=*.rs)
- `literal_argument_access_places`: 4 occ / 3 files; 1 external caller `flow_mutation.rs:3540`.
- `source_independent_place`: 5 occ / 4 files; `place_cases_are_selected_after` 5 occ / 3 files; `canonical_place_joined_segments_may_overlap` 5 occ / 3 files; `case_tag_segments_may_overlap` 5 occ / 4 files; `narrow_pinned_index_segments_with_values` 5 occ / 3 files.
- `place_case_path_is_inactive` 8 occ / 5 files; `place_case_has_value` 10 occ / 6 files; `place_cases_are_selected` 23 occ / 12 files; `CaseEvidence` 25 occ / 11 files; `stable_case_subject` 8 occ / 5 files.
- `coverage`/`domain_membership` modules only re-export; `segments_cover_subject` 15 occ / 7 files, `MembershipEvidence` 37 occ / 11 files (users exist outside).
- `canonical_place_from_symbol` 44 occ / 11 files; `literal_expression_projections` 26 occ / 12 files; `fixed_selection_segment` 12 occ / 8 files.
- Comment :1-5 says place projection "lives in validation's value-custody owner"; this file's re-exports of `validation::storage_path_relations` items (:1819-1824) have wide use, e.g. `place_segment_has_static_identity` 28 occ / 12 files.

### Stated limits
- "A selector the scalar facts cannot pin stays an `Index` segment, so the overlap predicates keep their conservative may-alias treatment." (:19-21)
- "Implicit zero formation has neither; expression/custody clients must fail closed." (:1413-1414)
- "Unknown stored values and malformed projections remain obligations." (:1450-1451)
- "Contrary tag evidence discharges the conditional obligation, but never authorizes reading that payload." (:1178-1179)

### Fail-open risks
- `place_cases_are_selected_after` :1163 returns `true` when no `Case` segment remains after the prefix (nothing to prove). Traced (trivial).
- `place_cases_are_selected_after` uses `.is_some_and(...)` so failed `CaseEvidence::prepare` yields `false` (fail-closed); `place_case_path_is_inactive`, `place_case_has_value` likewise false on failure. Fail-closed for reads; note `place_case_path_is_inactive` false means "obligation applies". None seen as accepting.
- `literal_value_path_is_inactive` returns false on any failed projection (fail-closed).
- `dispatch_copy_carries_case` :1012: accepts a case for a place when an *immutable* local copy of the same value has matching tag evidence (stated :1006-1011: "nothing can change it after the copy"). Stated. Condition checks the copy local is not mutable and its root is an immutable parameter or immutable local (`place_root_is_immutable`); it does NOT check that the source's storage is not reached through a mutable reference at an index/field below an immutable root (open; not traced).
- `Observer::predicate` :416-481: on an `||` with `value == true` or `&&` with `value == false` nothing is recorded (falls to the `Binary(binary)` arm, where `applied_case_membership_variant` returns None for non-membership) — drops evidence, fail-closed.
- `stable_subject` for `PlaceRoot::Expression(e)` returns true if `e.is_valid()` (:1241) with no further identity check. Wrong program: an expression-rooted place whose expression is not actually evaluated once. Status: open.

---

## 7. flow_reference_places.rs (1108 lines) — read in full

### Purpose
Resolves a reference-typed local, a reference-returning call, or a call actual to the concrete caller storage place(s) it names, so later proof/flow steps trace the value in the referent rather than in the reference binding. Header (:1-3): "Rebase a live reference before tracing the value currently in its referent." Also contains the "prefix gates" that decide whether earlier operand calls in the same statement could have disturbed that storage.

### Shape
- `mod result_candidates` (:16-754): `reference_result_candidates_before_statement` :47, `call_result_candidates` :82, `reference_expression_storage_places_including_shared` :136, `storage_places` :146, `argument_storage_places` :210, `storage_places_for_canonical_place` :220 (recursion depth budget 16 :202, :217; `depth == 0` -> None :229), `place_is_readable_reference` :534, `ReferenceResultSourceKind {Reference, Value}` :553, `ReferenceResultBacking {CallerArgument, InvocationResult}` :567, `ReferenceResultSource` :575, `call_result_leaf_sources` :588, `call_result_captured_sources` :608, `parameter_grants_readable_loan` :729.
- Top level: `assignment_preserves_reference_binding` :771, `reference_route_source` :789, `local_reference_storage_before_statement` :834, `local_reference_storage_at_call` :884, `local_reference_candidate_storages_at_call` :925, `call_position_in_state` :967, `preserve_call_prefix_storage` :995, `preserve_candidate_call_prefix_storage` :1061.

### Inputs / outputs
Reads the typed statement table, `CallFrameResolver` (`reference_result_transports`, `local_reference_origin_before_statement`, `*_reference_bindings_are_stable`), `FlowFacts` (`control.calls`, `normal_return_writes`), `BorrowFacts` calls. Outputs `Option<Vec<CanonicalPlace>>` / `Option<CanonicalPlace>`; `None` = keep conservative treatment (stated :44-45, :920-923).

### How it decides
- `reference_result_candidates_before_statement` :47-80: local must be declared before the statement, a readable mutable reference (`checks::contracts::is_readable_mutable_reference`), never reassigned as a whole symbol in the prefix (:63-69), and its initializer must be a call expression.
- `call_result_candidates` :82-130: each leaf source needs empty `result_path` and not `collection_coarse` (else `None` :94-96); `CallerArgument` requires the formal to grant a readable loan (:102) and resolves the actual via `storage_places`; `InvocationResult` becomes an `Expression(call_site)` root place. Empty -> None.
- `parameter_grants_readable_loan` :729-753: with `include_shared` false only readable mutable references; with true, any `Reference` whose access is not `WriteOnly` (:747); walks `Constrained`.
- `storage_places_for_canonical_place` :220-532: dynamic `Index` -> every fixed slot through `finite_projection_paths`, all must resolve (:232-267); Expression-rooted places unwrap Member/Indexed/Borrow/Match; a local declared before the statement that is itself a readable reference is traced through its initializer unless it was reassigned in the prefix (:362-410), otherwise `rebase_exact_local_place`; parameter that grants a readable loan -> rebase or itself (:499-524); else root must be a symbol and not a readable reference (:526-531).
- `call_result_captured_sources` :608-727: spelled `call.receiver.is_valid() != attached` check (:629), `carries_only_positional_arguments`, `selects_only_nominal_route`, arg count equals parameters; sources must not be image storage/recast/representation and segments only Field/Case/FixedIndex/Index (:670-684); source parameter found by `parameter.symbol == source.root` (:705-708).
- `call_position_in_state` :967: row identity = `(statement_index, call_ordinal, target_symbol, receiver_symbol)`; zero or >1 matches -> None.
- `preserve_call_prefix_storage` :995: for each earlier call in the same statement: `CallSite::Statement` -> `call_reference_bindings_are_stable`; `Expression` -> `expression_reference_bindings_are_stable`; `TransitionNamed` -> None; then `entry_origin_is_stable` (Contents) or `binding_origin_is_stable` (Binding) on the prior call's `normal_return_writes` (unknown when `!normal_return_writes_complete`).
- `preserve_candidate_call_prefix_storage` :1061: matches `borrow_calls` by statement/ordinal/target (:1081-1085), takes `signature_ceiling_places(program, machine, state, None, borrow_call, Some(frames))` and fails if any place may overlap a ceiling write (:1094-1104).
- `reference_route_source` :789: scans admitted execution order backwards from the statement for the latest `LocalData` initializer or whole-symbol `Assignment` that rebinds; an assignment that preserves the binding (`assignment_preserves_reference_binding` :771 — receiving type identity differs from declared) returns `None`.
- `local_reference_storage_before_statement` :834: exactly one `LocalData` declaration in the prefix (two -> None :861); non-reference local -> the place unchanged (:867-872).

### Cases by shape
- Match arms are explored for `Match` expressions (:176-191, :333-355). Member, Indexed, Borrow Expression roots handled (:269-357); all other expression kinds fall through to symbol rules.
- `call_result_captured_sources` handles only calls with `carries_only_positional_arguments() && selects_only_nominal_route()` (:630-631).
- `local_reference_candidate_storages_at_call` doc (:916-921) names the example `level.room_mut(cell)` giving 16 candidates `level.rooms[0..15]`; depth budget 16 in `storage_places_for_canonical_place` is a recursion bound not an element count.
- `preserve_candidate_call_prefix_storage` header comment: "the resolver's reference-binding walk deliberately declines once a divergent binding exists in the prefix" (:1056-1057).

### Same job elsewhere
- `reference_route_source` :789 and `local_reference_storage_before_statement` :834 and `storage_places_for_canonical_place` :362-410 each find "the initializer or latest rebinding of a reference local before a statement" with different scans (read all three).
- The assignment-to-whole-local test `place.root == PlaceRoot::Symbol(local) && place.segments.is_empty()` appears at :63-66, :372-378, :826-828 (three copies).
- `flow_mutation.rs:1882` `reference_result_candidate_places` consumes `reference_result_candidates_before_statement`; `flow_mutation.rs::place_requires_local_write_origin` (:1484) decides whether to ask for it.
- Suspected: `validation::CallFrameResolver::local_reference_origin_before_statement` also resolves local reference origins (used :873); not read.

### Unused / one-caller code (grep -rwn --include=*.rs; occ includes definition + re-export lines)
- `local_reference_candidate_storages_at_call`: 3 occ; 1 external caller `checks_contracts.rs:10506`.
- `local_reference_storage_at_call`: 7 occ / 5 files; callers `checks_contracts.rs:10495, :10555`, `checks_contracts_prover.rs:4513`, `checks_termination_progress.rs:1749`, `flow.rs:8370`.
- `assignment_preserves_reference_binding`: 5 occ; callers `checks_multiplicity_linear_validation.rs:1476` and in this file :819.
- `call_result_leaf_sources`: 5 occ / 2 files; external caller `flow_expression.rs:7206`. `call_result_captured_sources` 5 occ / 2 files.
- `argument_storage_places` 4 occ / 3 files; `reference_result_candidates_before_statement` 5 occ / 3 files; `call_position_in_state` 3 occ / 1 file; `reference_route_source` 9 occ / 5 files; `local_reference_storage_before_statement` 12 occ / 3 files; `reference_expression_storage_places_including_shared` 8 occ / 5 files.

### Stated limits
- "`None` keeps the conservative treatment." (:44-45)
- "one unresolved alternative fails the query." (:30-31)
- "the resolver's reference-binding walk deliberately declines once a divergent binding exists" (:1056-1057)
- "an absent or ambiguous identity declines rather than borrowing a same-shaped row's position." (:965-966)

### Fail-open risks
- `preserve_candidate_call_prefix_storage` passes `borrow = None` to `signature_ceiling_places` (:1090). With `borrow == None`, flow_mutation.rs:214-225 turns any by-value parameter whose `local_requires_write_origin` is true into `None` (`borrow.and_then(...)?`), so that gate declines — fail-closed direction. Traced (flow_mutation.rs:211-225).
- `storage_places_for_canonical_place` :526-531 final fallback returns `Some(vec![place])` for any symbol-rooted place that is not itself a readable reference and was not explained by a local/parameter branch. Wrong program: a symbol rooted place of an unknown kind (not a declared local before the statement, not a parameter) is accepted as its own storage. Status: open (not traced; symbols like machine fields likely).
- `local_reference_storage_before_statement` :858-860: no local declaration in the prefix -> `Some(place)` unchanged (treats the root as non-local, e.g. a parameter). Plausibly correct; open.
- `parameter_grants_readable_loan` for non-reference formals with `include_shared == true` returns false rather than true — fail-closed.
- `call_position_in_state` ambiguous -> None (fail-closed).

---

## 8. flow_retained_borrows.rs (1394 lines) — read in full

### Purpose
Computes the "handbacks" a normal consuming call owes: when a call completes and its borrow-carrying owned inputs end, which exclusive loans/places are returned to the caller and which facts (domain memberships) each returned place keeps, by following completed lifetime inputs through owned carriers. Header (:1-6): "A normal consuming call closes its exact actuals only after overlapping retained holders have all ended. Old scalar observations stay invalidated."

### Shape
- Top: `RetainedBorrowHandback` :18 {place, reference, supplies: `Vec<RetainedBorrowSupply>`} (conjunctive suppliers :21-24), `RetainedBorrowSupply` :29 {operand copy, constraints span}; `same_subject` :35, `require_supplies` :39.
- `mod qualifications` (:46-371): `RetainedMembership` :62 {place, domain, instance}; `memberships` :73; `replay_memberships` :156; `aliases` :235; `observation_aliases` :261 (cycle guard `active`).
- `mod captured_inputs` (:375-830): `RetainedSources {holders, borrows}` :401 with `append` :407 (union) and `meet` :425 (intersection); `captured_sources` :438 (inner `visit` :447, `profile_sources` :564); `reference_constraints` :703; `completed_calls` :748.
- `mod output_debts` (:831-1193): `sources` :850 (re-exported as `output_debt_sources`), `window_is_covered` :1076, `supplier_graph_is_known` :1149, `same_path` :1181.
- Top level: `ending_inputs` :1197, `handbacks` :1262.

### Inputs / outputs
Reads `BorrowFacts` (calls, loans, loan_suppliers, incoming_references, completed_reference_loans), `FlowControlFacts` (operand_retentions, operand_retained_suppliers/paths, output_lifetime_sources, calls), `FactPlan`, `FlowContextFacts`, `call_phases::ArgumentSources`. Outputs: `Vec<RetainedBorrowHandback>`, `Vec<RetainedMembership>`, `Option<RetainedSources>`.

### How it decides
- `ending_inputs` :1197-1260: call must be recorded identically (`borrow.calls.get(recorded) != call` -> None :1212), terminal callable target, and `boundary_requirement_completion == NormalReturn` (:1214-1215); then for each parameter that is not a returned-lifetime source, is not Unrestricted multiplicity, not a reference, and is borrow-carrying data (`is_borrow_carrying_data`) accumulate `sources.retained_sources(...)` (:1245-1258). For a statement call whose return type is a reference or borrow-carrying data -> None (:1236-1240).
- `handbacks` :1262-1393: keeps a returned borrow when its root is valid, all segments have static identity, and no other live `LifetimeDependency` loan overlapping that place still owes it: `produced_here` (formation expression == this call's authored expression) or (`formed_before(call)` and not ended by an earlier completed call's holders and (the other loan's last use is after the call, or at the call statement and not covered by this call's ending holders)) (:1368-1391).
- `memberships` :73-150: for each declared result-field domain path (static identity, valid instance) every supply must prove membership: `value_provable_domain(domain)` OR some alias place with cases selected and `evidence.contains(place, domain, instance)` (:113-126); surviving alias set is the intersection across supplies (`common.retain`). Keyed on domain symbol + semantic domain instance + place equality.
- `observation_aliases` :261-370: crosses stored reference bindings at the original snapshot; a capture's actual referee must have the same `normalized_type_identity` as the declared shell's referee (:322-326); aliases meet (intersection) across captures and across referent places.
- `output_debts::sources` :850-1068: requires non-reference, non-Unrestricted `contract`, a stable named holder not rooted at an Expression, `call.input_reference_loans` roster valid, every owner path's declared referent exclusive (:886), selected loans exclusive with lineage `SuppliedOutput` or `LifetimeDependency` only (:904-913), at least one output and one debt (:921-935), exactly one output lifetime receipt (:959-962), and each debt window covered by the original call's debts (`window_is_covered` :980). Restored borrows must match each debt's root and segments (`same_path`).
- `RetainedSources::meet` :425: keeps only holders/borrows present in both; a borrow missing from the other is dropped.
- Comparisons are by handle/identity, `same_path` via `canonical_place_segments_equal`, and `normalized_type_identity`; no string compares seen.

### Cases by shape
- `ending_inputs` Expression sites derive the "returned" parameter positions from `call_reference_lifetime_sources`; other (statement) sites accept only a return type that is neither reference nor borrow-carrying (:1234-1242).
- `captured_inputs::visit` is shaped by `exclusive_referent_type(contract)`: valid -> reference-handback branch; `reference_shell(contract)` -> empty result; else holders and owner paths (:463-561). In the reference branch only exactly one referent place (`[selected]`) yields a handback (:502).
- `reference_constraints` :703: a unique reference leaf at exactly the requested path length by segment equality, else `None`.
- `RetainedLifetimeInput::{Value, Incoming}` are the two input kinds (:608-638, :991-1020); `Incoming` with an exclusive referent bound -> None.

### Same job elsewhere
- The `match &mut common { Some(c) => c.meet(&x), None => common = Some(x) }` fold appears 8 times in this file (:495-498, :556-558, :593-595, :640-642, :664-666, :1022-1024, :1054-1056 and the alias folds :350-353, :359-362) (read).
- The Value/Incoming `retained_inputs` expansion is written twice: `profile_sources` :603-645 and `output_debts::sources` :986-1027 (nearly identical blocks; second calls `super::captured_sources` instead of `visit`).
- `window_is_covered` :1076 and `supplier_graph_is_known` :1149 both do an iterative DFS with `active`/`resolved` over `loan_suppliers` (read).
- `completed_calls::visit` :754 vs `flow_call_phases.rs:697` wrapper (`completed_calls` exists in both; the latter calls this one at :707).

### Unused / one-caller code (grep -rwn --include=*.rs)
- `output_debt_sources`: 2 occ; 1 external caller `flow_call_phases.rs:676`.
- `handbacks`: 3 occ; callers `flow_call_phases.rs:1875` and this file :213.
- `completed_calls` (this file's): 1 external caller `flow_call_phases.rs:707`.
- `replay_memberships`: 3 occ (def, re-export alias `replay_retained_memberships` at `flow.rs:6022`, one use via that alias not counted by this grep).
- `RetainedMembership` 8 occ / 3 files (`checks_content.rs:1818, :1846`); `RetainedBorrowHandback` 9 occ / 1 file; `RetainedBorrowSupply` 6 occ / 1 file; `ending_inputs` 3 occ / 1 file; `captured_sources` 7 occ / 3 files.
- No comment here calls anything "shared/canonical".

### Stated limits
- "Lifetime arcs transport obligations, never descriptor geometry or permission." (:3)
- "Neither relation manufactures an issuer." (:50-51)
- "Older shared ancestors are not a window identity" (:1074-1075)
- "Selections meet their possible inputs; strict evaluations retain call order." (:378)

### Fail-open risks
- `memberships` :113: `value_provable_domain(program, domain)` short-circuits the proof for such domains with no supply evidence (`proven = value_provable_domain(..) || aliases.any(..)`). Wrong input: a returned field whose domain is classed value-provable is credited as held without checking any actual input supply. Status: traced to its definition `checks_contracts_exits.rs:4379-4384`: `!domain_requires_provenance(domain)`, i.e. domains with no "established by" routes; its comment (:4372-4377) says such a domain is "re-provable from the field's value", so crediting it needs no input supply. Stated there; whether every consumer re-proves from predicates was not traced.
- `handbacks` :1271-1281: returns an empty list when `argument_sources` is `None`, `ending_inputs` is `None`, or there are no borrows. An empty handback list means no handback is recorded; effect on soundness depends on consumer (not traced). Open.
- `handbacks` filter :1368-1391: a returned borrow is dropped (not handed back) when a still-owing overlapping lifetime loan exists; the converse (loan not recognized) hands it back. Not traced; open.
- `ending_inputs` `continue`s over parameters that are Unrestricted/reference/non-borrow-carrying (:1246-1256): they contribute nothing. By design.
- `RetainedSources::meet` drops any borrow absent from the other alternative (fail-closed for handbacks).
- `observation_aliases` when stored snapshot capture is invalid -> `common = Some(Vec::new())` (no alias) :311-314 (fail-closed).

---

## 9. flow_state_values.rs (3815 lines) — read in full

### Purpose
During the flow builder's fixed point, computes what is known about each state's incoming parameters, attached-record fields, qualifications (domain memberships / float predicates), boolean predicates and selected cases by joining ("meeting") every arrival edge (transitions, invocations, private callable inputs, lexical regions), and publishes the settled rows as entry-context facts. Header (:1-5): "Same-invocation state arrivals transport live completed facts."

### Shape
- `mod arguments` (:24-168): `capture_argument` :42 — one completed jump argument to a `ScalarValue` (evaluates the checked scalar expression, else a literal, else a fixed-selection place read).
- `mod callable_inputs` (:169-481): `seed_contract_inputs` :185, `input_state` :214 (builds a `StateValues` from a callable's required signature: domain rows, `requires` expressions, memberships, cases, integer bounds), `fixed_path` :411, `rebind_place` :416, `rebind_predicate` :432.
- `mod lexical_regions` (:483-694): `record` :496 (captured entry values for an extracted region / nested machine / closed private machine).
- `mod parameter_bounds` (:695-804): `ParameterBounds` :719 {symbol, bounds, growth}; `WIDENING_DELAY = 4` :716; `join` :745 (union then widen to thresholds/carrier after 4 growths).
- `mod predicates` (:805-944): `meet` :822 (intersection), `capture` :829, `append` :935.
- `mod fields` (:946-1971): `BoundsSource {Transition, Invocation, CallableInput}` :968; `EdgeDelivery` :1014; `FieldValue` :1067 {root, segments, literal, predicates, integer_bounds, deliveries, bounds_growth, bounds_shrink}; consts `BOUNDS_WIDENING_DELAY = 4`, `BOUNDS_NARROWING_LIMIT = 8` :1103-1104; `height` :1106; `root_data_definition` :1132; `carrier_integer_range` :1151; `integer_literal_thresholds` :1195; `meet` :1247; `rejoin` :1311; `capture` :1460; `capture_completed_argument` :1505; `capture_storage_fields` :1563; `capture_predicate_bounds` :1771; `append` :1912.
- `mod qualifications` (:1972-2760): `QualifiedInput` :1996, `CarriedPredicate {Domain, Float}` :2011 (`implies`, `common_consequence`, `payload`), `subject_key` :2119 (64-bit mix hash of root and segments, `0x9E37_79B9_7F4A_7C15`), `deduplicate` :2169, `CapturedQualification` :2185, `meet` :2202, `height` :2252, `owned_projection` :2275, `capture_place` :2312, `capture_completed_argument` :2380, `capture_argument` :2464, `finish` :2555, `capture_invocation_referents` :2589, `append` :2698, unit test `subject_batches_preserve_order_and_evidence` :2724-2758.
- Top level: `StateValues` :2768 {state, values, bounds, fields, field_sources, qualifications, predicates, entry_paths, argument_edges, cases}; `ArgumentEdge` :2801; `ArgumentInputs {Unknown, Edges}` :2812; `entry_state_index` :2817, `entry_state` :2824, `reachable` :2833, `join` :2844, `unknown_inputs` :2962, `literal` :2993, `record_transition` :3008, `record_invocation` :3168, `capture_argument_cases` :3239, `completed_argument_matches` :3289, `CompletedArgumentFacts`/`completed_argument_facts` :3306/:3311, `selected_cases_under` :3365, `same_cases` :3420, `joined_cases` :3425, `caller_roster_supplies_entry` :3442, `lexically_nested_machine` :3455, `lexical_scope_covers_call` :3471, `append_entry_context` :3499, `InputDelivery`/`Delivery`/`RecordedVisit`/`InputRecording` :3595-3625, `delivers_here` :3632, `replay_delivery` :3678, `ReplayGuard` :3715 (+ unit test `outside_delivery_into_a_replayed_part_breaks_the_replay` :3769-3814).

### Inputs / outputs
Reads typed program tables, `FlowBuildContext` (control, contexts, operators, call_frames, state_value_inputs, replay_guard, input_recording), `FactPlan` semantic facts, borrow call facts. Mutates `build.state_value_inputs`, `build.dirty_state_value_inputs`, `build.new_state_field_input_height`, and (in `append_entry_context`) appends facts/contexts to `FactPlan` at `ProgramPoint::State`.

### How it decides
- A state has a row only after a delivery; "A missing row is unreachable. A present unknown value must participate in the incoming meet" (:2763-2765).
- `join` :2844: per-parameter scalar `values` meet to `Unknown` unless equal (:2891-2902); `bounds` union with widening; `fields` per-edge-delivery rejoin; `qualifications::meet` keeps only claims every edge supports (via `common_consequence`); `predicates::meet` retains predicates equal across arrivals; `entry_paths.union`; `cases` via `joined_cases` (intersection across edges :3425-3434).
- `record_transition` :3008: only `TransitionExit::Ordinary`, only a reachable source state, only `TransitionTargetNode::Named`, and not when the destination is the machine's entry state (:3044). A parameter's scalar value is kept only for integer-literal-accepting or `Bool` primitives and only if the completed operand matches `(expression, state_symbol, statement_index, type valid)` (`completed_argument_matches` :3289).
- `record_invocation` :3168: ordinary calls to non-entry states deliver all-`Unknown` values (:3214-3229); skips entry state and same-machine named transitions.
- Private callable inputs: `caller_roster_supplies_entry` :3442 = `!machine.is_public && !machine.target_family_interface && supplies_checked_body() && with_private_invocation_inputs(..).is_some()`. Caller facts enter a body "only when every possible invocation is known" (:3437-3440).
- `append_entry_context` :3499: an ordinary entry (no lexical region origin, not lexically nested) publishes nothing unless `caller_roster_supplies_entry` (:3509); otherwise publishes fields, qualifications, predicates, entry paths, scalar values (skip `Unknown`), integer bounds, cases.
- Widening: after 4 growths the join widens each extending endpoint to the nearest program comparison literal (`integer_literal_thresholds` :1195 — literals appearing as operands of `< <= > >= == !=` binaries) else the primitive carrier range; after 8 shrinks the shrunken axis is held (:1409-1421). Keyed on counters, not identity.
- `lexical_regions::record` `authenticated` :543-577: owner `caller_roster_supplies_entry`, call site target equals destination, and (for lexical region origin: `origin.machine/state` equal caller and argument expression handles `.eq` the origin's list; otherwise: argument roster equals captured operand roster and `lexical_scope_covers_call || !lexically_nested_machine`), no receiver, captured length equals parameter count, all captured at this statement.
- `lexical_scope_covers_call` :3471-3497 climbs `declaration_scope_parent` chain comparing `declaration.symbol`.
- Evidence strength: `QualifiedInput::same_subject` also requires `evidence.same_authority`; `finish` :2555 keeps a captured qualification only if `isolated` or (`bindings_stable` and the context is in the live constraint set).
- `fields::capture_storage_fields` :1563: facts `AssignedValue` (literal byte string or integer literal or assignment literal), `AssignedScalarValue` (Integer), `BytePredicate`, `AssignedIntegerBounds` with `min <= max`; targets only places whose suffix below an argument root is Field/Case/FixedIndex with valid symbols (:1679-1699); integer bounds ranked by `bound_point_rank` of the fact's `ProgramPoint` (:1572-1586).

### Cases by shape
- `capture_argument` (scalar) :58-102: only `TransitionTargetNode::Named` targets; role is `TransitionArgument` for the main target and `TransitionContinuationArgument` for the continuation, else Unknown (:96-102).
- `input_state` :243-261: signature formals must equal parameters in count (:240); contracts only `SignatureContractKind::Requires` (:263); proof facts `Expression`, `Membership`; `Proposition(_) => {}` ignored (:329).
- `fixed_path`/`rebind_place` :411-430: any `Index` segment -> not rebound.
- `selected_cases_under` :3365: only `CaseValue{value: true}` facts whose relative path is Field/Case segments only ("An index or range is meaningful only in the caller" :3362-3363); two facts selecting different variants at one place select nothing (:3409).
- `ByteSequencePredicate::ALL` is a fixed predicate list used at :1340, :1603, :1633 (defined in validation).
- Integer bounds -> byte predicates only when both bounds fit `u8` (:1655-1667).

### Same job elsewhere
- Interval join with widening is written three times: `parameter_bounds::join` :745-802 (symbol/parameter bounds, `WIDENING_DELAY`), `fields::FieldValue::rejoin` :1311-1427 (`BOUNDS_WIDENING_DELAY` + narrowing hold), and `lexical_regions::record`/`record_transition`/`fields::capture_completed_argument` hand-written min/max joins (:619-629, :3078-3097, :1546-1553). Two separate constants both 4: :716 and :1103.
- Interval intersection `max(min)/min(max)` copies at :361-364, :397-400, :1887-1891, `predicates` etc.
- "Capture predicates carried by a destination from caller places" in `qualifications::capture_place` :2312, `capture_completed_argument` :2380, `capture_invocation_referents` :2589, and `fields::capture_storage_fields` :1563 / `predicates::capture` :829 — four capture routines of the same shape for different payload channels.
- Common-across-alternatives fold (retain claims present in each alternative): :1530-1558, :2440-2446, :2541-2547, :3273-3275.
- `CarriedPredicate::from_fact` :2021 accepts only domain memberships and float predicates and rejects `FactOrigin::CallRequires` (:2022).
- Suspected elsewhere: `crate::published::facts::boolean_predicates` (BoundPredicate/EntryPaths) not read; `flow.rs` driver around :435-519 and :643 uses the exported functions (grepped only).

### Unused / one-caller code (grep -rwn --include=*.rs)
- `lexical_scope_covers_call`: 2 occ / 1 file (def + use :565).
- `record_transition`: 2 occ / 2 files (def + one call in flow); `unknown_inputs` 2 occ / 2 files (call `flow.rs:519`); `append_entry_context` 2 occ / 2 files (call `flow.rs:4658`); `replay_delivery` 2 occ / 2 files (call `flow.rs:643`); `record_invocation` 4 occ / 4 files (call `flow_calls.rs:56`; the other hits are an unrelated `record_invocation` method in checks_multiplicity_borrowed_windows.rs:1398); `seed_contract_inputs` 5 occ / 2 files (3 calls in `flow.rs:435, :483, :512`).
- `entry_state_index` 5 occ / 3 files (also `flow.rs:761`); `caller_roster_supplies_entry` 5 occ / 2 files; `lexically_nested_machine` 5 occ / 2 files; `integer_literal_thresholds` 5 occ / 2 files; `ReplayGuard` 10 occ / 3 files.
- `QualifiedInput` 22 occ / 1 file; `CarriedPredicate` 17 occ / 2 files; `InputDelivery` 5 occ / 1 file.

### Stated limits
- "Ordinary machine applications contribute unknown inputs to callable non-entry states; their incidental actuals never become assumptions of a generic body." (:2-4)
- "An open or public body stays checked only under its declared premises." (:486-487)
- "Growth counts are scheduling scratch, not proof evidence" (:707-708)
- "incoming joins can only weaken claims, and root entry is never narrowed." (:1979-1980)
- "An index or range is meaningful only in the caller, so it is not transported." (:3362-3363)
- Comment names a specific sample: "Squalr's byte-comparison loop kept the flow builder running for hundreds of thousands of sweeps" (:699-700) and "std's 193 literals made one loop counter take 183 passes, where its 81 compared literals suffice" (:1189-1190) — performance-motivated heuristics (widening thresholds).

### Fail-open risks
- `lexical_regions::record` :518-520, :533-541: returns without delivering (no `join` call) when the owner is neither nested/extracted nor closed-private, or when a `bounds_static_call` input already supplies the call. Absence of a delivery leaves the state's row as other edges made it; with no row, "A missing row is unreachable" (:2764) — a state whose only arrival was skipped here is treated as unreachable for facts. Status: open (not traced which callers rely on it).
- `record_transition` :3022-3026 and :3044: a non-Ordinary transition exit or one targeting the entry state delivers nothing. Wrong program: none shown; entry-state inputs come from the callers' roster. Stated by the doc :2-4 and by the entry-state comment at :3437-3440.
- `fields::meet` is skipped when `previous.argument_edges` is `Unknown` (:2873): fields stay as they were (`unknown_inputs` seeds `fields: Vec::new()`), so no field facts. Fail-closed.
- `value` meet :2891-2902: missing incoming row -> `Unknown`. Fail-closed.
- `append_entry_context` :3509: ordinary entry with no known caller roster publishes none — fail-closed.
- `input_state` :236-242: when `required_signature` errors or formals/parameter counts differ it returns the all-Unknown state without facts — fail-closed.
- `ProofFact::Proposition(_) => {}` :329 silently ignores propositional requires facts (not accepted as facts; just no fact). Fail-closed in this direction.
- `selected_cases_under` :3409 sets disagreeing variant to `None`: no case selected — fail-closed.
- `capture_argument` :103-137: when the checked scalar binding is absent (`None => {}`) it falls through to `literal(program, argument)` or a place read — i.e. value from a literal or the current place value. Stated by role-keyed lookup; not traced further.
- `expect` panics: `"recording checked above"` :3659, `"replayed parts exceed u32"` :3727.

---

- **Coverage:** all nine files read in full, in order, with the Read tool: flow_mutation.rs 1-4762 (4762 lines), flow_observations.rs 1-1690 (1690), flow_operand_contexts.rs 1-1136 (1136), flow_operand_values.rs 1-1193 (1193), flow_ownership.rs 1-1989 (1989), flow_place.rs 1-1837 (1837), flow_reference_places.rs 1-1108 (1108), flow_retained_borrows.rs 1-1394 (1394), flow_state_values.rs 1-3815 (3815). Total read in full 18,924 lines; skimmed 0; not read 0. Besides the unit I read only `checks_contracts_exits.rs:4370-4400` (to trace `value_provable_domain`) and the grep hit lines named above. Counts come from `grep -rwn --include=*.rs <name> .` run from `omega-rust` (`wc -l` for occurrences, `grep -rlw ... | wc -l` for files); "occ" includes definitions, `use` lines and re-exports. The `grep -w` matches also hit comments and same-named items in other modules (flagged where noticed).

- **Five things a maintainer most needs to know about this unit.**
  1. `flow_mutation.rs` is one 4,762-line file of nested modules (`ceiling`, `local_origins` with `coordinates`, `operand_coordinates`, `receiver`, `regions`, `summary`) and its output type `StorageRegion` has four variants (`Declared`, `UnresolvedContent`, `ResidentContent`, `NonresidentContent`); `StorageRegion::declared_places` (:2308) returns `None` if any region is not `Declared`, so callers confined to declared coordinates fall into the "`None` = drop every context" path rather than omitting content writes. Per-state mutation summaries are computed once per program into a `OnceLock` by a rayon dirty-mask fixed point (:2659-3141); a state is usable only if `complete`.
  2. Several functions re-resolve a write-frame report path by splitting a dotted `String` and matching names: `place_from_origin_path` (flow_mutation.rs:1928-1975) matches the first member by `parameter.name`/local `name`, and `frame_write_place` strips a literal `"[*]"` suffix (:1924). That is name-based resolution of rendered paths inside an otherwise symbol-keyed module. Operator resolution in flow_ownership.rs passes `call.target.as_str()` and textual static-path segments (:1083-1090, :1398-1405, :1360-1366).
  3. `flow_state_values.rs` is the flow fixed point's input join: widening/narrowing counters (4 growths, 8 shrinks) and program-literal thresholds decide where intervals settle; the join logic for intervals exists in three separate implementations with two separate constants (:716, :1103). Missing row = unreachable; present Unknown participates in the meet.
  4. Evidence is authenticated by exact occurrence identity (state symbol, statement index, call ordinal, expression handle, arena handle+generation) and fails closed on ambiguity (e.g. flow_observations.rs `completed_case_subject` returns a named refusal reason for each failed gate; flow_reference_places.rs `call_position_in_state` declines on ambiguous rows); the two functions I found that accept on missing evidence are listed under fail-open (`descriptor_context_agrees` with zero rows, `statement_storage_writes`'s `.or(Some(places))`).
  5. Many `pub(crate)` helpers here have exactly one external caller (e.g. `call_receiver_write_accesses`, `call_mutated_regions`, `linear_restoration_ceiling_regions`, `output_debt_sources`, `completed_dispatch_extraction`, `local_reference_candidate_storages_at_call`, `owned_call_operand_places`), while a few are broadly shared (`operand_binds_live_view` 48 occ / 16 files; `CapturedValue` 141 occ / 10 files; `call_permission_evaluation` ~14 external call sites). `family_attached_data_symbol` (flow_mutation.rs:2177) and `rebase_descriptor_slot_regions` (:1671) are used only inside their own file.

- **What you are unsure about.**
  - Whether `statement_storage_writes`'s `.or(Some(places))` at flow_mutation.rs:4612 can ever see `close_storage_places_over_aliases` return `None` for a program that matters (I did not trace what makes alias closure `None` after `assignment_storage_places` already succeeded).
  - Whether `append_call_ownership_events` silently skipping unresolved callees (flow_ownership.rs:67-69) is unreachable because earlier checks reject unresolved calls; and whether `ownership_place_from_expression_in_state` returning `None` (:869-874, :909-911) can occur for an accepted program.
  - The `value_provable_domain` credit in `memberships` (flow_retained_borrows.rs:113): I traced its definition, not every consumer of `RetainedMembership`.
  - Whether `stable_subject` accepting any valid `PlaceRoot::Expression` (flow_place.rs:1241) can hold an expression evaluated more than once.
  - Behaviour of `validation`-crate helpers named throughout (`CallFrameResolver`, `completed_storage`, `membership_evidence`, `storage_path_relations`); I read only their call sites. "Same job elsewhere" items marked suspected were not read at the other site.
  - Counts include definition and re-export lines; I did not separately count test-only uses.

