Second pass for unit 59

Ranges covered by this note (gap_017):
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs` lines 7201-11937 (4737 lines), read in full.

Abbreviation: F = `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs`. All paths below are under that `src/` unless stated.

Where the range starts: line 7201 is in the middle of `fn context_survival` (7184-7257), a function inside the `invalidation` sub-module of the module that closes at F:7291. The re-exports at 7284-7290 and the later `pub(crate) use domain::...` at 11732-11735 show this enclosing module is `domain` (inferred; I did not read its header, which is before my range). The range ends at the end of the file (11937).

Contradiction with the earlier notes: none. The earlier notes say 1201-11937 was not read and describe it only from the header; the header list (state_values, calls, exits, mutation, transfers, reach...) matches the `#[path]` modules and `use` lines I saw (F:7292-7293 `#[path = "flow_mutation.rs"] mod mutation`, 8161-8166 `flow_ownership.rs`, `flow_place.rs`, `flow_reference_places.rs`, 11713-11730 imports).

Ranges in the file by item (sub-ranges of 7201-11937):
1. 7201-7291 tail of `domain::invalidation` (context survival after a write) + re-exports
2. 7292-7293 `mutation` path module declaration
3. 7295-7532 `jump_loans`
4. 7533-8155 `receiver_reads`
5. 8157-8166 path module declarations (`ownership`, `place`, `reference_places`)
6. 8167-8931 `value_origins`
7. 8933-8984 `constraints`
8. 8986-9378 `reference_spans` (with one test)
9. 9380-9872 `carried_semantic_dependencies` (with one test)
10. 9873-10514 `cleanup_case_evidence` (+ nested `completed_results`)
11. 10515-11434 `cleanup_effect_dependencies`
12. 11435-11692 `dynamic_effect_dependencies`
13. 11693-11832 re-export block, `shared_call_frames_or`, `resolved_operator_statement_symbol`, `call_ensures_point`
14. 11834-11937 `local_jump`, `terminal_invocation`

---

## 1. `domain::invalidation` tail, F:7201-7291

### Purpose
Finishes `context_survival`, which decides what is left of one semantic context (a set of facts that held at a program point) after a place was written. Facts whose place overlaps the write are removed; a storage-dependency fact and the fact it guards are removed together; everything else stays because "independent field domains are not one proposition" (comment 7179-7183).

### Shape
- `enum ContextSurvival { Unchanged, Dropped, Reduced(Vec<FactHandle>) }` F:7173-7177.
- `context_survival` F:7184-7257, takes a `matching_mutation` closure and an `invalidated` callback.
- `dependency_is_strict_prefix_of_dependent` F:7264-7281.
- Re-exports at 7284-7290: `build_domain_facts`, `qualification_is_content_independent`, `relative_place_segments_from_expression`, `context_survives_place_mutations`, `filter_contexts_after_mutations`, `filter_contexts_after_place_mutations`, `matching_mutation_for_fact_place`.

### Inputs and outputs
Reads a `FactPlan` (facts, places, place_segments, contexts). Returns a `ContextSurvival`; calls `invalidated(handle, mutation_place, dependency_segments)` for each fact removed by a matching write (7225). No mutation of the plan.

### How it decides
- A context holding any `StorageDependency` fact with no place (`fact.place.as_place().is_none()`) is dropped entirely (7200-7206).
- Per fact with a place, `matching_mutation(semantic, fact, place)` (supplied by the caller) decides overlap (7214). The match is keyed on place identity, not names.
- Dropping a `StorageDependency` also drops its `dependent` unless the dependency place is a strict segment-prefix of the dependent's place (7219-7224, 7264-7281: same root, longer segments, `starts_with`). Comment 7259-7263: "`main.pause` does not change `main.name`".
- A second loop (7230-7245) drops any StorageDependency whose dependent was dropped (to fixpoint).
- If everything is dropped -> `Dropped`; else `Reduced(survivors)` (7246-7256).

### Cases by shape
None (generic over places).

### Same job elsewhere
Not seen in my range. Callers of the re-exports `filter_contexts_after_mutations`, `matching_mutation_for_fact_place`, `qualification_is_content_independent`, `relative_place_segments_from_expression`: Grep count mode over `omega-rust` found 36 occurrences in 6 files (F 23, flow_mutation 2, flow_expression 1, flow_call_phases 5, checks_contracts_writes 3, checks_contracts_prover 2); not broken out per name.

### Unused or one-caller
Not measured per name (combined count above). `context_survival` and `dependency_is_strict_prefix_of_dependent` are private to the module.

### Stated limits
None.

### Fail-open risks
- 7211-7213 / 7269-7271: a fact with no place is skipped (`continue`) so it survives. Reached by any non-storage-dependency fact without a place (e.g. a pure predicate). Not "traced"; the comment at 7179-7183 says non-overlapping facts stay by design: "stated".
- 7221: when the dependency is a strict prefix of the dependent, the dependent is kept after the guard is retired. Stated at 7259-7263.

---

## 2. `jump_loans`, F:7295-7532

### Purpose
For a call that is a terminal invocation (a transfer that completes a transition arm), works out which of the target state's accesses count while each incoming argument loan is still carried. One footprint is kept per reference-bearing incoming parameter (7309-7311).

### Shape
- `HeldAccesses<Access> { parameter: SymbolHandle, accesses: Vec<Access> }` 7333-7337.
- `held_accesses` 7339-7350 (copies the whole direct-access list under each loan parameter) and `loan_parameters` 7352-7368.
- `enum TargetRun { Whole, Forwarded(Vec<ParameterLoanSources>) }` 7370-7373; `ParameterLoanSources { parameter, is_data_binding, sources: Option<Vec<CanonicalPlace>> }` 7375-7380 ("None is an unresolved source, not absence of a loan").
- `TargetRun::accesses` 7383, `new_accesses` 7393, `forwards` 7427.
- `target_run` 7473-7531.

### Inputs and outputs
Reads `CheckingInput` (state parameters, `terminal_invocation`, `find_state_in_machine`, `family_data_binding_parameter`) and `mutation::instantiate_call_relative_places`. Produces a `TargetRun`; `accesses` returns an iterator of which stored accesses apply.

### How it decides
- `loan_parameters` keeps a state parameter unless `validation::has_contents(.., NumericOwned)` is true (7360-7366): a parameter that is purely numeric-owned carries no loan. Keyed on the parameter's type.
- `target_run` returns `Whole` when there is no terminal invocation (7479-7482).
- `forwards` (7427-7470): no `Forwarded` -> true; no binding row for the parameter -> true; unresolved sources (`None`) -> true; loan whose root is not a valid `PlaceRoot::Symbol` -> true; otherwise requires same root, not (data-binding with a shorter source whose segments are all equal to the loan's prefix and no loan segment has unresolved identity), and `canonical_place_segments_may_overlap`.
- Sources are rejected (set to `None`) if any instantiated source root is not a valid symbol, or is neither the machine symbol nor a parameter of the caller state (7506-7519).

### Cases by shape
- `is_data_binding` special case (7448-7462) applies only to the data-binding parameter of the family (`family_data_binding_parameter`, 7485-7490).

### Same job elsewhere
- `jump_loans` is shared with `receiver_reads` (F:7585) and with `flow_mutation.rs:2451` (grep: `held_accesses`/`target_run` used at flow_mutation.rs:2549, 2608, 2693, 2776). So this is a shared helper used by two footprint fixed points; the fixed point in `receiver_reads::settle_receiver_call_reads` (F:7827) and the one in `flow_mutation.rs` have the same shape (suspected duplicate; I did not read flow_mutation.rs).
- Users outside this file's range: grep `jump_loans::|target_run\(|held_accesses\(` gives 2 uses in F (receiver_reads) and 5 in flow_mutation.rs.

### Unused or one-caller
All functions are `pub(super)`; each used by both `receiver_reads` and `flow_mutation.rs` (target_run 2+3 call sites, held_accesses 1+1).

### Stated limits
- "Multiple reference leaves within one parameter currently share its footprint" (7317-7318).
- "An unresolved origin conservatively retains every possible incoming loan; it never proves release" (7314-7315).

### Fail-open risks
none seen: every unknown path in `forwards` returns `true` (keeps the access), which is the conservative direction. Stated 7314-7315.

---

## 3. `receiver_reads`, F:7533-8155

### Purpose
Computes, for each state, the may-read footprint of storage reached through its data binding, so a whole `&mut self` receiver call can be judged by what the callee reads rather than treated as one exclusive operand. Footprints are cached per program and built lazily (7600-7607).

### Shape
- `const MAXIMUM_PLACE_DEPTH = 32` 7598.
- `StateReceiverReadCache { states: OnceLock<Vec<StateReceiverReads>> }` 7601-7607 (`pub(crate)`).
- `StateReceiverReads { state_symbol, complete: bool, reads, held_reads }` 7610-7618.
- `call_receiver_read_accesses` 7628-7684 (`pub(crate)`, the only query).
- `ReceiverCall` 7690, `FootprintOwner` 7701, `DelegatedReceivers` 7962, `DirectReads` 7969.
- `build_state_receiver_reads` 7706-7820, `settle_receiver_call_reads` 7827-7883, `extend_new` 7886, `FootprintOwner::{instantiate 7904, reaches 7939}`.
- `DirectReads::{collect 7979, record 8021, value 8028, place 8100, selectors 8142}`.

### Inputs and outputs
Reads typed state statements, expression table, `BorrowFacts`, `place_at` (published places), the `mutation` module's `call_data_argument`, `canonical_receiver_place_for_call_site`, `call_receiver_mutated_place`, `instantiate_call_relative_places`, `AccessRouteReader`. Produces `Vec<CanonicalPlace>` of reads in the caller's coordinates.

### How it decides
- `call_receiver_read_accesses`: no target state, no footprint entry, or an incomplete footprint -> the whole receiver place (7647-7658). Otherwise picks the target's whole-run or held-run reads via `target_run(...).accesses(...)` (7659-7665) and instantiates each at the call; any read that cannot be instantiated -> whole receiver (7676-7678).
- Footprint is keyed on state symbols and `CanonicalPlace` roots. `reaches` (7939-7954): the place root equals the machine symbol or the family data-binding parameter symbol. Comment 7567-7568: "Named parameter roots never collapse to the machine symbol".
- `complete` is false when the machine has no body, a state parameter has no valid type reference (7724-7728), a call site cannot be found (7739), a receiver cannot be placed (7768), or a `SavedSubject` has no authenticated home (8129).
- `DirectReads::collect` (7979-8019): a call statement's receiver is recorded unless delegated; each expression root of a statement is classified by `StatementRootRole` — `AssemblyFact` ignored, `AssignmentTarget` read as a place with `named = false` (selectors only), delegated `TransitionArgument` the same, everything else `value`.
- `DirectReads::value` (8028-8094) is an exhaustive match on `ExpressionNode` (placement or recursive). `place` (8100-8140) records a place only if rooted at a `PlaceRoot::Symbol`.
- Depth: instantiated places longer than 32 segments widen to the call's receiver (7925-7929).
- Settling: monotone union to fixpoint (7833-7882); stops when nothing grew.

### Cases by shape
- Only calls that are terminal invocations and have a data-binding argument are "delegated" (7742-7760). Other calls are read as ordinary operands (inferred from `value` handling `Call` at 8042-8060).
- `ExpressionNode::TypeExpression(_)` and spelled literals read nothing (8091-8092).

### Same job elsewhere
- Mirrors the mutation-summary fixed point in `flow_mutation.rs` (uses the same `jump_loans` helper, flow_mutation.rs:2451-2776). Suspected duplicate shape; not read.
- `held_accesses`/`target_run` usage: see section 2.

### Unused or one-caller
- `call_receiver_read_accesses`: one outside call site (`checks_borrows_calls.rs:765`). `StateReceiverReadCache`: outside uses at checks_borrows_calls.rs:708/714, checks_borrows.rs:5476/5497/6208/11427 (grep content mode).
- `extend_new` (7886), `DirectReads::*`: private.

### Stated limits
- "A target without a summarized body ... contributes the whole receiver place" (7558-7559).
- "A state whose own body or receiver call cannot be placed has no footprint, and its callers treat their whole receiver as read" (7560-7561).

### Fail-open risks
- `call_receiver_read_accesses` 7637-7646: `whole_receiver()` returns an EMPTY vec when `call_receiver_mutated_place` returns `None`. Comment at 7625-7626: "Empty when the receiver itself has no place." A call whose receiver has no place reads nothing for the borrow comparison. "stated".
- 7747-7749 and 7761-7770: a terminal-invocation call without a data argument is `continue`d; it is neither delegated nor given a receiver call. It is then read by the ordinary operand walk, not by a footprint. "traced" (read `DirectReads::value`, 8042-8060, which handles the call receiver and arguments).
- `place()` 8110-8116: for a symbol-rooted place with `named == false`, nothing is recorded (only selectors). Stated at 8096-8099 ("`named` records the place itself").
- `place()` with a `PlaceRoot::Expression` root (a captured home) is treated as a computed base and recurses (8117-8138); `Name` -> nothing recorded (8137). A `Name` expression that does not resolve to a symbol-rooted place through `place_at` records no read. "open": I did not read `place_at`.

---

## 4. Path module declarations, F:8157-8166 and 7292-7293
Declares `#[path]` modules `ownership` (flow_ownership.rs), `place` (flow_place.rs), `reference_places` (flow_reference_places.rs), `mutation` (flow_mutation.rs). Those files are not in my unit. Comment 8157-8160: "queries also used outside `flow`".

---

## 5. `value_origins`, F:8167-8931

### Purpose
Traces a place backward through the preceding statements of a state to find where its value came from at a call or statement boundary: through owned stores (replacing the place with the stored value's source), through local-data initialisers, and through constructor literals; it fails (returns `None`) when an unknown write intervenes.

### Shape
- `CAPTURED_SOURCE_LITERAL_HOPS = 16` 8199.
- `enum ValueObservation { Contents, Binding }` 8206-8210 (`operand_writes` 8219, `statement_writes` 8235, `changed_by` 8251).
- `enum OriginFrontier { CompletedStatement, CapturedValue }` 8212-8216.
- Entry functions (all `pub(crate)`): `value_origin_at_call` 8269, `value_origin_before_statement` 8300, `value_origin_at_call_resolving` 8347, `value_origin_matches_call_return` 8399, `trace_value_origin_before_statement` 8451. Core: `trace_value_origin_observing` 8487-8668.
- Helpers: `captured_source_place` 8678, `result_call` 8765, `reference_storage_or_literal` 8807, `exact_suffix` 8826, `preserve_statement_effects` 8841, `preserve_frame` 8908.

### Inputs and outputs
Reads `CheckingInput`, `FlowFacts` (published statements/operands: `entry_origin_writes`, `entry_origin_unknown_write`, `completed_writes`, etc.), `CallFrameResolver`, `flow::{statement_mutated_place, statement_storage_writes, local_reference_storage_before_statement, rebase_exact_local_place, frame_storage_writes, literal_expression_projections}`. Outputs `Option<CanonicalPlace>`; `None` = unknown/not preserved.

### How it decides
- `ValueObservation::changed_by` (8251-8266): same root AND `canonical_places_may_overlap` for `Contents`, or `binding_overwrite` for `Binding`. Keyed on place identity (root + segments).
- Backward scan over `admitted_execution_order` positions (8521-8660). For an assignment: if it replaces a local reference binding (8538), only effects are preserved; else computes the target place; if the demanded place has the target as an exact prefix (`exact_suffix` 8826), the store is "decisive": requires `captured_assignment_destination_remains_current` (8580), a declared stored type (8587), and `proof_value_is_caller_isolated` (8593); then continues from `captured_source_place`. Otherwise any write of the statement overlapping the place returns `None` (8610-8624).
- `LocalData` whose symbol is the demanded root is handled the same way (8626-8645).
- Every statement then runs `preserve_statement_effects` (8648-8659). With published flow it reads the stored fact row (`find(statement_index)?`, 8854-8859) and judges `entry_origin_is_stable` / `binding_origin_is_stable`; without published flow it uses the resolver's `may_write_frame` and `statement_value_write_frame` (8884-8905). Comment 8834-8840: missing published rows never fall back to resolver frames.
- `value_origin_matches_call_return` (8399): a callback `observe` fires at each frontier; matched only if the returned place, rebased, equals the demanded place and `call_return_before_statement_agrees` (8436-8440); else `false`.

### Cases by shape
- `captured_source_place` (8678-8757): requires exactly one literal projection (8697-8700). Follows cast chains; a result-position call asks the caller-supplied `resolve`; a `StructLiteral` or an `ArrayLiteral` indexed by `PlaceSegment::FixedIndex` continues into the selected element; "Any other expression keeps the whole projection" (8676, 8753). Depth capped at 16 hops, then `None` (8703, 8756).
- `result_call` (8765): peels `Member` and `Indexed` only, then expects `Call`; anything else -> `None`.
- ArrayLiteral with a non-FixedIndex first segment and non-empty path -> `None` (8740-8747).

### Same job elsewhere
- `exact_suffix` (8826) vs. `ValueObservation::changed_by`: different questions. The same "is this place covered by that write" logic also appears in `reference_places` and `mutation` (not read; suspected).
- Pre-frontier rebase and observe hooks are generic callbacks; consumers use different hooks (checks_termination_progress passes `resolve`).

### Unused or one-caller
Grep `value_origin_at_call\b|value_origin_before_statement|value_origin_at_call_resolving|trace_value_origin_before_statement|value_origin_matches_call_return` over `omega-rust` (content mode): outside-F callers: `value_origin_matches_call_return` 2 (checks_contracts_prover.rs:1514, 7077); `value_origin_at_call` 3 (prover 1741, 1944; checks_contracts.rs:10591); `value_origin_before_statement` 2 (prover 2070; checks_content.rs:4010); `trace_value_origin_before_statement` 2 (checks_termination_progress.rs:2549, 3126); `value_origin_at_call_resolving` 1 (checks_termination_progress.rs:4170). All have at least one user outside F.

### Stated limits
- "an unresolved call stays opaque" (8675); "this query neither re-executes a call nor treats a reference as an owned copy" (8296-8297).
- "For consumers without a rebase hook the literal spelling still fails closed at the first reference-typed declaration" (8804-8806).

### Fail-open risks
- `reference_storage_or_literal` (8807-8824): `.unwrap_or(source)` keeps the literal place when reference storage cannot be resolved. Stated at 8797-8806; the `proof_value_is_caller_isolated` check (8593, 8629) and later frontiers' write fences are what catch it. "stated".
- `captured_source_place` 8753 `_ => return Some(source)`: an unrecognised expression keeps the whole projection, i.e. the trace continues from the expression's own place. "stated" at 8676.
- `trace_value_origin_observing` 8646 `_ => {}`: statement kinds other than Assignment/LocalData go straight to `preserve_statement_effects`, which fails closed via `?`. "traced" (read).
- `preserve_statement_effects` with no published flow and a non-call statement only checks `statement_value_write_frame` (8896-8904), no check of other effect kinds. "open": I did not read `statement_value_write_frame`.
- `rebase(state, successor, &place)` returning `None` leaves the place unchanged (8533-8535): stated by the doc (8334-8346: "None to keep the literal spelling").

---

## 6. `constraints`, F:8933-8984
Two functions, `append_contiguous_borrow_root_constraints` (8947) and `append_contiguous_borrow_access_constraints` (8966), each appends one `FlowConstraintRef` per handle of a borrow-fact span. Early return if the span start is invalid (8953-8955, 8972-8974). Same shape twice (differ only in the `FlowConstraintKind` variant and handle type). Outside use not grepped separately (imported into F at 11722-11724).

---

## 7. `reference_spans`, F:8986-9378

### Purpose
The single place for append / filter / intersect rules on the flow facts' reference spans (semantic-context refs, constraint refs, place segments). Header 8987-8995.

### Shape
- Generic helpers: `intersect_references_in_order` 9005, `sorted_reference_identities` 9018.
- Context membership: `semantic_context_identity` 9028, `SemanticContextMembership` 9037 (`new` 9042, `contains` 9050), `intersect_semantic_contexts_in_order` 9057.
- Constraints: `constraint_reference_identity` 9069 (kind tag 0-7 plus arena index and generation), `meet_constraint_references_in_order` 9093.
- Spans: `retained_flow_contexts` 9129, `retained_constraint_refs` 9214, `append_flow_reference` 9148, `appended_span_since` 9156, `append_place_segments` 9163, `append_flow_contexts_for_points` 9174, `append_flow_contexts` 9193, `append_semantic_context` 9229, `append_constraint_ref` 9249, `project_constraint_refs_to_active_contexts` 9257, `filter_constraint_refs` 9287, `filter_flow_references` 9295.
- One `#[cfg(test)]` test (9325-9377).

### Inputs and outputs
Mutates `arena::PagedArenaSet<..>` reference arenas and `HandleSpan`s; reads `FactPlan` for context handles.

### How it decides
- Identity is the full generational handle: `(arena_index, generation)` (9028-9030, 9069-9085). Comment 9001-9002: "Scratch keys never become published identities".
- `meet_constraint_references_in_order` (9093-9127): semantic constraints are intersected; `BorrowLoan` and `BorrowIncomingReference` entries are unioned (the "debts join", 9087-9090). All other kinds (BorrowState/Call/WritableRoot/Access, SemanticContext, Unknown) are only kept if in both sides.
- `project_constraint_refs_to_active_contexts` (9257-9285): a `SemanticContext` constraint survives only if its exact handle is among the active contexts (page-by-generation lookup in a `BTreeMap`, 9263-9273, 9278-9281); `Unknown` and every borrow-resource constraint are always kept (9282).
- `filter_flow_references` (9295-9323): keeps contiguous runs as sub-spans of the source, and copies to a new span only when the kept set stops being contiguous.

### Cases by shape
None.

### Same job elsewhere
- `retained_flow_contexts` (9129) and `retained_constraint_refs` (9214) are the same function over two reference types.
- `append_flow_contexts` (9193) and `append_semantic_context` (9229) both append a context to the context-ref arena and a `SemanticContext` constraint to the constraint arena; the former loops over handles, the latter creates the context first.
- `append_flow_reference` (9148) just calls `append_to_span`, `appended_span_since` (9156) just calls `span_since`, `filter_constraint_refs` (9287) just calls `filter_flow_references`: pass-through wrappers.
- `append_contiguous_borrow_*` (section 6) have the same shape as each other.

### Unused or one-caller
Grep count over `omega-rust` for `intersect_semantic_contexts_in_order|meet_constraint_references_in_order|SemanticContextMembership|retained_flow_contexts|appended_span_since|append_flow_reference|filter_flow_references|filter_constraint_refs|append_flow_contexts_for_points|append_semantic_context`: 111 occurrences in 9 files, all in `flow*.rs` (flow.rs 56, flow_expression 15, flow_call_phases 15, flow_exits 11, flow_observations 4, flow_calls 4, flow_transfers 3, flow_context 2, flow_state_values 1). Specific users read by grep (content): `intersect_semantic_contexts_in_order` and `meet_constraint_references_in_order` each 1 outside call (flow_expression.rs:11654, 11672); `SemanticContextMembership::new` 1 outside (flow_state_values.rs:2572); `retained_flow_contexts`/`retained_constraint_refs` many (flow_call_phases, flow_calls, flow_transfers, flow_exits, and F).
Comment "the rules ... live here instead of being restated at each producing site" (8993-8994): users outside this module exist in 7 other files, so the claim of being shared holds.

### Stated limits
- "Callers supply a validated selection or a list built here. Revalidating its entire prefix on every append would turn list construction quadratic" (9145-9146): appends are not re-validated.

### Fail-open risks
- 9282: `FlowConstraintKind::Unknown | borrow_resource_pattern!(..) => true` in the active-context projection: borrow-loan constraints are kept regardless of which contexts are active. No comment at the site says why. Reached by any constraint list carrying a loan row when contexts are narrowed. "open" (I did not trace what a surviving loan constraint means for a consumer).
- 9133-9137 / 9218-9222: empty span -> `HandleSpan::empty()`, otherwise the source is returned as is. Not an accept/reject judgment.

---

## 8. `carried_semantic_dependencies`, F:9380-9872

### Purpose
Rebuilds the table of nominal declarations and cleanup machines that each machine depends on, with exposure (public interface vs private implementation), from signatures, call return types, ownership events and cleanup plans of the complete check facts. Comment 9382-9386: not a flow-build step; derived after execution finalization.

### Shape
- `DependencyRows { rows: Vec<SemanticDependency>, positions: BTreeMap<(u32,u32,u32,u32,Kind), usize> }` 9407-9410.
- `derive_checked_semantic_dependencies` 9419-9543 (`pub(crate)`).
- `append_type_dependencies` 9545, `event_statement_index` 9585 and `event_type_reference` 9606 (`pub(super)`, reused by `cleanup_effect_dependencies`, 10526), `collect_nominal_symbols` 9655, `push_nominal_symbol` 9704, `event_exposure` 9726, `machine_signature_exposure` 9756, `push_cleanup_machine` 9770, `push_promoting` 9787.
- Test `indexed_promotion_matches_ordered_linear_accumulation` 9808-9871.

### Inputs and outputs
Reads `CheckingInput` and `CheckFacts` (`flow.control.states/calls`, `flow.ownership.permissions/segments`, `flow.terminal_unit_effects`, `flow.terminal_partial_affine_unit_cleanups`). Returns `CheckedSemanticDependencies { rows }` sorted by (consumer arena index, dependency arena index, kind, exposure) (9534-9542). Comment 9417: the retained `facts.flow.semantic_dependencies` rows "are deliberately not consulted".

### How it decides
Five sources of rows (in order):
1. Entry state of each machine: parameter types and return type, exposure from `machine_signature_exposure` (9425-9448).
2. Each call in the flow control facts: the callee's return type, `PrivateImplementation` (9450-9466).
3. Each permission event: the type at the event's place, exposure from `event_exposure`, `automatic_cleanup = (kind == AffineDrop)` (9468-9497).
4. Normal-exit cleanup actions from three plan lists (9499-9527): `AutomaticCleanupMachine` rows.
5. For each nominal dependency: three kinds `NominalIdentity, Layout, OwnershipBehavior` (9556); plus `AutomaticCleanup` and, if `data_cleanup_machine` exists, an `AutomaticCleanupMachine` row (9567-9581).
- Dedup keyed on (consumer handle with generation, dependency handle with generation, kind); a later `PublicInterface` promotes an existing row (9787-9806).
- Exposure judgment: public if the machine `is_public` or its `supply_mode.is_boundary_declaration()` (9737, 9762); in `event_exposure` additionally the state must be the first state and the root must be the machine or an entry-state parameter (9744-9750).
- Nominal collection (9655-9702): unwraps Reference, Constrained, FixedArray, Slice, Generic (base symbol plus arguments), DynamicTrait (symbol plus arguments), Named; skips `BuiltinType`, `TypeParameter`, `ConformanceParameter`, `MachineParameter`, `PropositionParameter`, `PropositionMachineParameter` (9709-9718).

### Cases by shape
- Only the first state of a machine is its "entry" for the signature rows (9426, 9741).
- `TypeReferenceNode::ConstExpression | Unit` contribute nothing (9700).

### Same job elsewhere
- `event_exposure` (9726) and `machine_signature_exposure` (9756) both test "machine is public or boundary declaration" with different lookups (`machines_with_symbol(..).find` vs `machines().iter().any`).
- `event_statement_index` also exists with a different signature in `checks_multiplicity_linear_validation.rs:4071` (takes `FlowOwnershipEventSource`, returns `Option<usize>`). The two are different functions with the same name and a similar job (suspected duplication of the source-to-statement mapping; I did not read the other).
- Grep `event_statement_index|event_type_reference|machine_signature_exposure|event_exposure` count: F 13, the rest in checks_multiplicity* files (these may be the other same-named function).

### Unused or one-caller
- `derive_checked_semantic_dependencies`: grep content over `omega-rust` shows outside callers at `checking.rs:1183`, `products.rs:66/70` (a public wrapper), `lib.rs:70` re-export, and `omega-rust/omega/packages/review-evidence/src/capture/package_policy/semantic_dependencies.rs:24/32`. So it has 3 real call sites outside F (checking.rs, products.rs, review-evidence).
- `event_statement_index`, `event_type_reference` (pub(super)): used inside F by `cleanup_effect_dependencies` (10657, 10662, 10771) and this module (9472).

### Stated limits
None beyond the header.

### Fail-open risks
These are all "a missing row means a missing dependency":
- 9426-9428: machine with no states skipped. "open".
- 9452-9456: call whose target return type cannot be found is skipped (`continue`). "open".
- 9469-9471: permission event whose machine is not found skipped. "open".
- 9474-9482: event whose type reference cannot be found skipped. "open" (a dependency edge for that event is not published).
- 9732-9753: `event_exposure` returns `PrivateImplementation` when the root is not a symbol, the machine is not found as public, there is no entry state, or the state is not the entry. This under-reports exposure; a later `PublicInterface` can promote. "open".

---

## 9. `cleanup_case_evidence`, F:9873-10514

### Purpose
Answers "is this enum case reachable at this cleanup occurrence?" for owned cleanup. Combines authenticated exclusions from the permission schedule (`ContextualCleanupDisposition`) with case facts from the semantic fact plan and from the selected arms of a `Match` that produced the value. Header 9875-9886.

### Shape
- Nested `completed_results` (9895-10265): `CaseSelections` 9911 (`new` 9918), `SelectionRoute` 9974, `CompletedResult` 9980 (`contexts` 10126, `belongs_to` 10145, `proves_case` 10158), `CompletedResultNode` 9991, `CompletedResults` 9997, `selected_results` 10001-10123.
- `CleanupCaseEvidence<'a>` 10278-10286: `new` 10289, `at_completed_result` 10344, `definitely_transferred` 10357, `contexts` 10361, `excludes` 10376, `proves_case` 10382, `reachability` 10411-10507.
- Re-exports 10266-10268 and 10510-10514 (renamed `CleanupCaseSelections`, `CompletedCleanupResult`, `CompletedCleanupResults`, `cleanup_result_selections`).

### Inputs and outputs
Reads `CheckFacts`/`CheckFactsRef` (contextual cleanup exclusions, flow control operands, owned case selections), `place_case_has_value` and `place_case_path_is_inactive` (from the `place` module), `occurrence_contexts`, `validation::completed_storage::contents_preserved_from`. Returns `CleanupCaseReachability` (`Selected`, `Inactive`, `Runtime`) or `Option` of it.

### How it decides
- `CleanupCaseEvidence::new` (10289-10342) returns `None` if any exclusion's root differs from the disposition root or has cause `InactiveCase`, or any inactive-case entry is not rooted at the disposition root, not `InactiveCase`, or does not end with a `PlaceSegment::Case`; definite exclusions must also appear in the exclusion set.
- `reachability(path)` (10411-10507): a path with no `Case` segment -> `Selected`; path starts with an `inactive` entry -> `Inactive`; all occurrence contexts say the path is inactive -> `Inactive`. Then for each `Case{variant}` segment, using the prefix as subject: if `proves_case(.., variant, false)` -> `Inactive`; if `proves_case(.., variant, true)` -> continue; else look up the closed data definition of the subject type; if any OTHER variant is neither excluded nor proved absent -> `Runtime`; if the variant is not declared -> `None`; if there is no other variant -> `Runtime`. End -> `Selected`.
- `proves_case` in `completed_results` (10158-10263): for every captured value, either `place_case_has_value` holds, or walking the match route upward, an `owned_case_selection` row of the same machine, state, statement, evaluation, expression, root and path proves it. The current arm proves the variant when `row.variant == variant && required`, or a sibling variant of the same parent when `!required`; a prior arm only proves absence (`prior && row.variant == variant && !required`, needs `miss_excludes_case`). Also requires `contents_preserved_from` (10247-10255).
- Keyed on symbol and handle identity (`variant` symbols, arena index+generation), not names. `program.symbols.get(row.variant).parent` equality at 10241-10242 uses the symbol's parent link.

### Cases by shape
- `selected_results` (10001-10123): each captured operand must share expression, state, statement index and evaluation, and evaluation must not be `Unknown` (10037-10044); only a `Match` expression whose completion is `Selected` creates branches (10025-10030). Alternatives must be actual arms of the dispatch, unique, and their value operand's expression must equal the authored arm's value (10063-10078). Else `None`. Bounded by a counter `remaining` of operands + alternatives + 1 (10014-10020).

### Same job elsewhere
- `CleanupCaseEvidence::proves_case` (10382) and `completed_results::proves_case` (10158) both call `place_case_has_value`; one over occurrence contexts, one over capture contexts.
- Uses `validation::completed_storage::contents_preserved_from` (10247) while `dynamic_effect_dependencies` uses `stored_values::at_snapshot` (11541); both are re-exports of the same `validation::completed_storage` (F:1376 for `stored_values`).

### Unused or one-caller
Grep count `CleanupCaseEvidence|cleanup_result_selections|CleanupCaseSelections|CompletedCleanupResult` over `omega-rust`: 14 occurrences, in F (7), `execution_terminal_unit_cleanup.rs` (6), `checks_multiplicity.rs` (1). Outside users exist (2 files).

### Stated limits
- "Independently supplied or persisted rows need that same authentication" (9886).
- "an unknown exit tag alone supplies no such evidence" (10355-10356).

### Fail-open risks
- `reachability` returns `Inactive` when `proves_case(.., false)` holds (10463): this drops cleanup for that case. The wrong program that gets through would be one where a case-absence row is wrongly authenticated. "traced" partly: absence is proved only via contexts or an owned case selection tied to the same statement/evaluation/expression/root/path and `contents_preserved_from` (10219-10255).
- `CaseSelections::new` (10918-10960) and `selected_results` return `None` on any malformed row, so a failure removes evidence rather than adding it. none seen.
- `contexts()` returning `None` makes `proves_case` false and path-inactive false (10167-10169, 10428-10444). Fail-closed.
- `Runtime` is returned when the subject type has no other variant (10502-10504): a single-variant enum is treated as always present. Derives from the type; not a risk.

---

## 10. `cleanup_effect_dependencies`, F:10515-11434

### Purpose
For each contextual cleanup disposition (a dying owned value at some point), walks the type to find which nominal cleanup machines run for the remaining (not excluded or transferred) part of it, and turns those into call-graph edges (`CleanupDependency`). Also supplies the type-side eligibility check for whole or residual owned death edges and the canonical order for cleanup.

### Shape
- `enum CleanupLeaf { Nominal{machine,path}, Symbolic{parameter,path} }` 10576-10585.
- `concrete_cleanup_paths` 10546 (expands `FixedRange` into `FixedIndex` paths, with `try_reserve_exact`).
- `selected_cleanup_leaves` 10591, `selected_cleanup_leaves_in` 10600-10684.
- `whole_cleanup_is_eligible` 10693, `residual_cleanup_is_eligible` 10701.
- `derive_cleanup_effect_dependencies` 10727-10817 (`Result<Arena<CleanupDependency>, Vec<Diagnostic>>`).
- `visit` 10853, `visit_field_child` 10824, `visit_owner` 11006, `visit_array` 11214, `resolve_peeled_type_reference` 10981, `erased_subject_needs_disposition` 11189, `selected` 11204, `select_type` 11282, `applied_owner` 11372, `cleanup_path_order` 11388, `segment_order` 11397.

### Inputs and outputs
Reads `ContextualCleanupDisposition` rows (`facts.contextual_cleanup`), ownership segments, `CleanupCaseEvidence`, type reference table, data definitions and members, `closed_array_extent`, `nominal_cleanup_machine_for_type`, `has_contents`. Returns leaf lists / eligibility booleans / dependency arena / diagnostics.

### How it decides
- `selected_cleanup_leaves_in` (10600-10684): returns `Some(vec![])` if any removed (excluded) path is a prefix of the selected `path` (10614-10616) or `disposition.zero_image` (10617-10619). If an excluded path is neither prefix nor extension of `path`, but `storage_path_compatibility(...).0` is true, and `captured_storage_paths_are_disjoint` does not prove disjointness, returns `None` (10624-10652). Type is `disposed_type` if valid, else derived via `event_type_reference` and `select_type` (10658-10664).
- `visit` (10853-10973): empty removed path -> done; peels constraints and substitutes named type parameters (`resolve_peeled_type_reference`); a `Reference` or `Slice` yields no leaves (10869-10874); recursion through `active` stack returns `None` (10875-10877). `FixedArray` with a closed extent -> `visit_array`; with an open extent but declared extent and nothing removed -> visits element 0 and accepts only if it has no leaves and no erased obligations (10896-10916); else `None`. `Named`/`Generic` -> `visit_owner`. `Unit`, `ConstExpression`, `DynamicTrait` -> no leaves (10964-10968).
- `visit_owner` (11006-11181): resolves the data definition by `applied_owner` (closed, then application, then by symbol); a builtin type -> done; an open `TypeParameter` with nothing removed -> a `Symbolic` leaf; any other symbol kind -> `None`. A `Linear` data type without a nominal cleanup machine -> `None` (11034-11036). A nominal machine adds a `Nominal` leaf, but only if every removed sub-path is inactive (11037-11049). Then visits fields (skipping erased fields, which set `has_erased_obligations` if the subject needs disposition) and variants. For a variant with no removed paths, it conservatively tries visiting payload fields with `inactive_case = false`; if that gives no leaves and no erased obligation, the variant is skipped (11100-11128). Else `inactive_case(path)` (11130) skips an inactive variant. Finally `matched == removed.len()` or `None` (11180): every removed path must be consumed by a field/case/segment.
- `derive_cleanup_effect_dependencies` (10727-10817): first loop errors if any call from a CheckedBody non-proof machine targets a machine with open parameters (10735-10757). Second loop over dispositions, skipping dispositions of machines that are not CheckedBody, have open parameters, or are proof machines (10759-10768). `selected_cleanup_leaves(...)` `None` or a `Symbolic` leaf -> error diagnostic naming machine and state (10790-10798, message at 10784-10788, with a hint at 10780 when the path has an `Index` segment). A target machine with no entry state or not found -> `continue` (10799-10804). Edges deduplicated by linear `any` (10811).
- `segment_order` (11397-11433): ordinal found by scanning ALL data definitions for the field/variant/payload-field symbol (11404-11426); `FixedIndex` uses its index; `FixedRange`, `OpaqueRange`, `Index` -> `None`. Returns `usize::MAX - ordinal` so later declared members sort first (reverse declaration order).

### Cases by shape
- `visit_array` (11214-11276): removed paths must start with `FixedIndex` (index < length) or a one-segment `FixedRange` within length; anything else `None` (11228-11238).
- `select_type` (11282-11363): only `FixedArray` (indexed by `FixedIndex` in range or a non-range `Index`), `Named`, `Generic`; segments `Field`, or `Case` followed by `Field`; erased fields -> `None`.
- `Generic` requires `parameters.len() == arguments.len()` (10941-10943, 11320-11322).

### Same job elsewhere
- `whole_cleanup_is_eligible` is `residual_cleanup_is_eligible` with empty removed/inactive (10693-10698).
- `visit_field_child` (10824) wraps the push/visit/pop pattern used at 11061-11071 and 11154-11164.
- The "peel constraints and chase substitutions" loop at 10981-11004 duplicates in spirit the type-unwrapping helpers in `validation` (`unwrapped_type_reference`, used at 10477); suspected, not read.
- `cleanup_path_order` (11388) is "shared by prerequisite checking and executable partitions" (11383-11386). Grep content: users outside F: `published_facts.rs:1265, 2087`, `execution_terminal_unit_cleanup.rs:2784`. So the shared claim holds (3 outside sites).

### Unused or one-caller
Grep (content mode) over `omega-rust` for `cleanup_path_order|concrete_cleanup_paths|whole_cleanup_is_eligible|residual_cleanup_is_eligible|selected_cleanup_leaves_in|selected_cleanup_leaves`: outside-F call sites: `selected_cleanup_leaves` 3 (published_facts.rs:896, 1200; checks.rs:8881); `selected_cleanup_leaves_in` 1 (execution_terminal_unit_cleanup.rs:251); `concrete_cleanup_paths` 2 (published_facts.rs:2084; execution_terminal_unit_cleanup.rs:414); `cleanup_path_order` 3 (published_facts.rs:1265, 2087; execution_terminal_unit_cleanup.rs:2784); `whole_cleanup_is_eligible` 1 (execution_terminal_unit_calls.rs:4840); `residual_cleanup_is_eligible` 1 (checks_multiplicity.rs:3056). `derive_cleanup_effect_dependencies`: 1 outside call (published_facts.rs:11206).

### Stated limits
- "This is a may-effect set" (10523-10524); "repeated array elements contribute one target edge, not invented call sites" (10525).
- "This is not proof of the selected hooks' requirements or guarantees" (10687-10692).
- Diagnostic text: "runtime-indexed case observation names no reusable case fact: bind a borrow of the selected element before testing its case" (10780).

### Fail-open risks
- 10614-10616: an excluded prefix returns no leaves, so no cleanup dependency is recorded for transferred or absent storage. Stated by the module header (10520-10521: "transferred or absent subtrees are excluded").
- 10617-10619: `zero_image` returns no leaves with no comment at the site. "open" (not traced what produces `zero_image`).
- 10869-10874 and 10964-10968: `Reference`, `Slice`, `DynamicTrait`, `Unit`, `ConstExpression` produce no cleanup leaves. A `DynamicTrait` value that has an owner cleanup would have no edge from this walk. "open".
- 10799-10804: a nominal leaf whose machine has no entry state is skipped. "open".
- 10896-10916: an array with an open extent is accepted when element 0 has no leaves and no erased obligations. "stated" by code shape, since an element with no leaves needs no cleanup.
- 10665-10670: an unresolved reachability (`None`) is treated as NOT inactive, so more cleanup is expected. Fail-closed.
- 11124-11127: a variant whose payload fields visit conservatively with no leaves and no erased obligation is skipped; fine because it has nothing to clean.
- 10759-10768: dispositions of machines that are not `CheckedBody`, have open parameters, or are proof machines are skipped with no diagnostic. "open".

---

## 11. `dynamic_effect_dependencies`, F:11435-11692

### Purpose
For calls that pass a `Selector`-typed argument, finds which implementations (or entry parameters) that argument can come from, so a dynamic call's effect envelope can be narrowed to those implementations. Header 11436-11439: narrows "through completed selector values, never current storage. Unknown predecessors retain the public envelope".

### Shape
`derive` 11449-11498 (`pub(crate)`, returns `CapturedSelectorCalls`), `selected_implementations` 11500-11621, `returned_implementations` 11623-11691.

### Inputs and outputs
Reads `collect_dynamic_implementation_selections(program)`, flow control calls/operands/exits, `completed_values::predecessors`, `stored_values::at_snapshot`. Produces `CapturedSelectorCalls { origins, arguments, calls }` arenas.

### How it decides
- Only arguments whose type is `PrimitiveType::Selector` are considered (11460-11464). If `selected_implementations` is `None` (unknown) or empty, the argument is skipped (11465-11477), so the consumer keeps the full envelope.
- `selected_implementations`: cycle on `(operand, projection)` -> empty (11511-11516). An empty projection whose operand expression IS a recorded implementation selection -> that implementation (11520-11528). A `Name` expression that is an incoming parameter with no stored value at the snapshot: in the entry state -> `Parameter(name.symbol)` (11557-11560); in a later state -> union over all arrival calls into that state at the parameter's position, `None` if there are none (11561-11589). Otherwise follows `completed_values::predecessors`, and if there are none, `returned_implementations`; an empty predecessor list -> `None` (11599-11601).
- `returned_implementations` (11623-11691): exactly one call flow matching the authored expression and statement index; target machine must not be public and must be `CheckedBody`; the call target must be the entry state or the machine; union of every exit's returned value of that machine, mapping `Parameter` origins to the actual argument's origins. `None` if no exits or empty.

### Cases by shape
- Only the `Selector` primitive; only `Name` expressions or recorded selections, or completed-value predecessors or returns of private checked bodies; "no exit/empty" -> `None`.

### Same job elsewhere
- Parameter-to-actual mapping (11561-11589 and 11673-11680): both walk `call.argument_operands().handles().nth(position)`.

### Unused or one-caller
`dynamic_effect_dependencies::derive`: 1 outside call (published_facts.rs:11208). Module is `pub(crate)` and referenced only by path (`crate::flow::dynamic_effect_dependencies::derive`).

### Stated limits
"Unknown predecessors retain the public envelope; every known alternative contributes" (11437-11438).

### Fail-open risks
none seen in the direction of accepting: unknowns return `None` and the caller keeps the broader envelope (stated 11437). The cycle return `Some(Vec::new())` (11515) contributes no origins; "open" (no comment; it is sound only if another path provides an origin, otherwise the argument is skipped by the `origins.is_empty()` check at 11475).

---

## 12. Re-export block and small free functions, F:11693-11832

### Purpose
Wires the sub-modules' items into `crate::flow`'s namespace; defines three small helpers.

### Shape
- `shared_call_frames_or` 11772-11782: returns the shared `CallFrameResolver`, or builds one into `owned`. Comment 11761-11771: a `None` result "means exactly what a failed private construction meant".
- `resolved_operator_statement_symbol` 11784-11790: wraps `operator_calls::resolve_operator_statement_call`.
- `call_ensures_point` 11797-11808: builds `ProgramPoint::CallEnsures { machine_symbol, state_symbol, statement_index, call_ordinal }`.
- Re-export blocks at 11693-11759 and 11810-11832 for modules `cleanup_effect_dependencies`, `reference_places`, `value_origins`, `borrow_lifetimes`, `boundaries`, `builder`, `call_phases`, `calls`, `carried_semantic_dependencies`, `constraints`, `context`, `domain`, `exits`, `mutation`, `operator_calls`, `receiver_reads`, `reference_spans`, `ownership`, `place`, `reach`, `state`, `statements`, `transfers`.

### Unused or one-caller
Grep count `shared_call_frames_or|resolved_operator_statement_symbol|call_ensures_point` over `omega-rust`: 50 occurrences in 16 files (F 9). All three are used outside F (checks_termination_progress 5, checks_contracts 4, checks_operators 3, checks_contracts_calls 3, flow_mutation 7, ...); not per name.

### Fail-open risks
none seen.

---

## 13. `local_jump` and `terminal_invocation`, F:11834-11937

### Purpose
Two lookups that classify a borrow call. `local_jump` identifies a transfer to a non-entry state of the SAME machine (internal block boundary). `terminal_invocation` identifies the call that completes an authored transition arm and gives its target state (argument transport only; "not caller-scope retirement", 11897-11899).

### Shape
`LocalJump { target, site }` 11843, `local_jump` 11848-11890; `TerminalInvocation { target, site }` 11901, `terminal_invocation` 11906-11935.

### How it decides
- `local_jump`: the target must be a state of the machine and not its entry (11856-11860). A `TransitionNamed` site qualifies when the path has exactly one member and its head resolves (`call_target_entry_symbol`) to the target (11869-11872). An `Expression` site qualifies when the transition arm completes (`completes_transition`), the call has no runtime receiver for the target, and the target entry symbol equals the target (11873-11886). Other sites -> `None`.
- `terminal_invocation`: needs a call site, a transition arm that is valid and `completes_transition`, and `call_target_entry_symbol(site.target_symbol(..)) == target.symbol` (11914-11934).
- Keyed on symbols and call-site handle (`statement_index`, `call_ordinal`), not names.

### Cases by shape
- `local_jump`: `site` is only `TransitionNamed` with exactly one path member, or `Expression`. 11869 "count() == 1".

### Unused or one-caller
- `local_jump`: outside the module, `crate::flow::local_jump` is wrapped by `checks_borrows.rs:6402-6407` and its wrapper has 7 call sites (checks_borrows_calls.rs:210, 483, 502, 938; checks_borrows.rs:6265, 6422, 6673). `LocalJump` re-imported at checks_borrows.rs:6400.
- `terminal_invocation(`|`TerminalInvocation`: Grep count 17 occurrences in 6 files: F 5, execution_terminal_unit_state_graph 1, execution_terminal_unit_control_statement_sequence 4, execution_terminal_unit_control 3, checks_borrows_calls 1, and `omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_scalar_graph.rs` 3. The `omega` hits are probably unrelated same-named items; I did not read them (suspected, not verified).

### Stated limits
- "Entering another machine still keeps caller storage and loans alive until ordinary completion; only `local_jump` can establish an internal block boundary" (11898-11899).

### Fail-open risks
none seen: every missing piece returns `None`.

---

## Coverage
- Read in full: `flow.rs` lines 7201-11937 = 4737 lines.
- Skimmed: none.
- Not read: none.
- Extra lines read above the range for context: 7160-7200 (not counted).
- Greps run (all over `omega-rust`, Grep tool, content or count mode): `derive_checked_semantic_dependencies`; `dynamic_effect_dependencies|derive_cleanup_effect_dependencies`; `cleanup_path_order|concrete_cleanup_paths|whole_cleanup_is_eligible|residual_cleanup_is_eligible|selected_cleanup_leaves_in|selected_cleanup_leaves\b`; `call_receiver_read_accesses|StateReceiverReadCache`; `value_origin_*`; `\blocal_jump\b|LocalJump`; `terminal_invocation\(|TerminalInvocation`; `shared_call_frames_or|resolved_operator_statement_symbol|call_ensures_point`; the reference-span helper names; `CleanupCaseEvidence|...`; `filter_contexts_after_mutations|...`; `mod stored_values|mod completed_values|...` in flow.rs; `event_statement_index|...`; `fn (held_accesses|target_run|...)`; `jump_loans::|target_run\(|held_accesses\(`; `flow\.rs` in `tools/dense_id_maps.txt`.

## Five things a maintainer most needs to know about this range
1. Most of F:7201-11937 is query code over completed facts, not the flow builder itself: footprint fixed points (`receiver_reads`, F:7533), backward origin tracing (`value_origins`, F:8167), dependency tables rebuilt from complete check facts (`carried_semantic_dependencies`, F:9380, "deliberately not consulted" retained rows 9417) and cleanup walks (`cleanup_effect_dependencies`, F:10515). Their consumers are in `checks_*`, `published_facts.rs` and `execution_terminal_unit_cleanup.rs` (greps above), so a change here changes check results, not flow rows.
2. `jump_loans` (F:7300) is one helper shared by `receiver_reads` and `flow_mutation.rs` (greps: 2 and 5 uses). Both footprint fixed points are built from it; `forwards` (F:7427) returns `true` (keep access) in every unknown case.
3. `value_origins` fails closed through `?` on every unknown, but keeps a literal place spelling when reference storage cannot be resolved (`reference_storage_or_literal`, F:8807, stated at 8797-8806) and keeps the whole projection for unrecognised captured expressions (F:8753). The safety then rests on `proof_value_is_caller_isolated` (F:8593, 8629) and the write fences.
4. Cleanup walking (`visit`, F:10853) is a fixed list of type shapes: references, slices, dyn traits, unit and const expressions yield no cleanup leaves (F:10869-10874, 10964-10968), open type parameters yield `Symbolic` leaves that `derive_cleanup_effect_dependencies` turns into an error (F:10793-10798), linear data without a nominal cleanup machine is refused (F:11034-11036).
5. `reference_spans` is the shared home for context and constraint span rules (users in 7 other `flow*.rs` files), but `project_constraint_refs_to_active_contexts` keeps every `Unknown` and borrow-resource constraint regardless of active contexts (F:9282, no comment), and `meet_constraint_references_in_order` unions loan debts while intersecting everything else (F:9087-9127).

## What I am unsure about
- The header of the module that encloses lines 7201-7291 is outside my range; I inferred it is `domain` from the re-exports (F:11732-11735).
- I did not read `flow_mutation.rs`, `flow_place.rs`, `flow_reference_places.rs`, `flow_ownership.rs` or `validation::completed_storage`, so several "same job elsewhere" claims (the mutation-summary fixed point mirroring `settle_receiver_call_reads`; `place_case_has_value`'s behaviour; `statement_value_write_frame`) are suspected, not verified.
- I did not trace what `ContextualCleanupDisposition.zero_image` means (F:10617), nor whether the 3 `omega-rust/omega/...` hits for `terminal_invocation(|TerminalInvocation` are the same items.
- BTreeMap use in this range (F:9263, 9409, 9913-9914, 9994): tools/dense_id_maps.txt lists only `control_flow.rs` for the `omega` crate in my grep (`flow\.rs` pattern matched one line, unrelated), so I did not establish whether this file is on the allowlist or checked by that rule; I did not read the rule.
- Per-name caller counts for the re-exports at F:7284-7290 are combined, not broken out.
