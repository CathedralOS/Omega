# slice_0016

Area digest preamble (read): `digests/area_09.md` — stage 05 flow (`flow.rs` and fifteen `flow_*.rs` files) walks execution order, publishes `FlowFacts`, invalidates semantic facts on writes, receiver-read footprints, constraint projection, semantic-dependency derivation, cleanup leaves, and captured selector origins. Words: open / stated / traced / suspected. Citations pinned at `457adf4a11f`; cited excerpts were re-read in the current checkout at the line numbers below (no relocation needed for these rows).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0117 | flow.rs:6609-6641 | CLOSED | `qualification_is_content_independent` is the sole gate before content-write qualification preservation; callers use it at invalidation and operand admission. |
| A0118 | flow.rs:6913-6920 | REACHABLE | `matching_mutation_for_fact_place` returns no match for `Unknown` roots, so placed facts (including operator `ensures` at `Unknown`) survive arbitrary writes. |
| A0119 | flow.rs:8110-8138; flow.rs:8896-8904 | REACHABLE | Unplaceable `Name` adds no receiver-read footprint while `complete` can stay true; without published flow, origin checks use only the value write frame. |
| A0120 | flow.rs:9282 | UNDECIDED | `Unknown` and borrow-resource constraints are always retained when projecting to active contexts; effect on final accept/reject was not traced through constraint consumers. |
| A0121 | flow.rs:9426-9482; flow.rs:9732-9753 | CLOSED | Failed machine/call/event lookups only omit optional semantic-dependency rows; stage-05 checking does not treat a missing row as acceptance. |
| A0122 | flow.rs:10617-10619; flow.rs:10759-10804 | UNDECIDED | `zero_image` and filtered machine kinds skip cleanup leaves/edges by design; whether those flags can be set incorrectly on an invalid program was not read. |
| A0123 | flow.rs:11515 | UNDECIDED | Selector cycle returns empty origins and skips capture for that operand; whether effect inference or a later check rejects the program was not read. |
| A0124 | flow_call_phases.rs:836-857 | REACHABLE | `retain_guarantees` treats non-parameter contract occurrences as surviving any mutation, so stale `ensures` contexts can remain after call writes. |

## A0117

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:6609-6641` — inner `visit` returns `false` when the domain has classification, semantic roles, operators, a present predicate with empty proof facts, failing alias walk, or failing `proof_facts` membership/expression checks; otherwise pushes `symbol` to `completed` and returns `true`.

**Enclosing function (read):** `qualification_is_content_independent` (`flow.rs:6590-6646`) with module comment at `flow.rs:6583-6588` (content vs descriptor coordinates).

**Callers (search):** pattern `\bqualification_is_content_independent\b` in `omega-rust` — 1 definition (`flow.rs:6590`), 1 re-export (`flow.rs:7285`), 3 production uses (`flow_mutation.rs:2373`, `flow_call_phases.rs:1287`, `flow_expression.rs:5404`); 0 test-only matches.

**Upward trace (read):** `preserves_owner_qualification` (`flow_mutation.rs:2344-2376`) uses `qualification_is_content_independent` only on the `NonresidentContent` arm to treat an owner as preserved without invalidating on content writes. Call invalidation (`flow_call_phases.rs:1274-1305`) calls `preserves_owner_qualification` before `matching_mutation_for_fact_place`. Operand domain admission (`flow_expression.rs:5400-5408`) skips provenance/dependency work when content-independent.

**Input that reaches the branch:** A domain definition with no classification, operators, roles, predicate, aliases, or proof facts, so `visit` returns `true` at `flow.rs:6638-6640`.

**Closing check:** Every path that skips content-driven qualification invalidation reads `qualification_is_content_independent` at the call sites above; the predicate at `flow.rs:6609-6641` is the only classifier for that behavior (read).

## A0118

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:6913-6920` — early `return None` when `place.root` is `Unknown`, an invalid symbol, or an invalid expression handle.

**Enclosing function (read):** `matching_mutation_for_fact_place` (`flow.rs:6901-7007`) in `invalidation::matching`.

**Callers (search):** pattern `\bmatching_mutation_for_fact_place\b` in `omega-rust` — 1 definition (`flow.rs:6901`), 1 re-export (`flow.rs:7290`), 4 call sites (`flow.rs:7030`, `flow.rs:7097`, `flow_call_phases.rs:1296`, `flow_call_phases.rs:2059`, `flow_call_phases.rs:2084`); count 5 listing lines including re-export use in `flow_call_phases.rs:2084`.

**Upward trace (read):** `visit_context_invalidations` (`flow.rs:7043-7073`) calls `matching_mutation` only when `fact.place.as_place()` is `Some`; a `None` match leaves the fact uninvalidated. Operator `ensures` with no operand places are published at `FactPlace::Unknown` (`flow.rs:4432-4449`, read). `filter_contexts_after_place_mutations` (`flow.rs:7075-7107`) drops contexts when any fact is invalidated.

**Input that reaches the branch:** A semantic fact with a place handle whose root is `facts::PlaceRoot::Unknown` (read: published at `flow.rs:4434` for dependency-free operator ensures).

**Smallest program (plain words):** An operator `ensures` clause whose operands have no resolvable places is published at an unknown place; a later statement write anywhere in the machine does not invalidate that ensures fact, so checking may still treat the guarantee as live.

## A0119

**Cited sites (read):** `flow.rs:8110-8138` — `DirectReads::place`: symbol-rooted places may `record`; expression-rooted bases set `captured_home`; `ExpressionNode::Name(_)` is a no-op. `flow.rs:8896-8904` — `preserve_statement_effects` without `flow` applies `statement_value_write_frame` only (after optional `may_write_frame` for calls).

**Enclosing functions (read):** `place` (`flow.rs:8100-8140`) inside `receiver_reads` / `DirectReads`; `preserve_statement_effects` (`flow.rs:8841-8906`).

**Callers (search):** `DirectReads::collect` only from `build_state_receiver_reads` (`flow.rs:7788-7796`, read). `preserve_statement_effects` — grep `preserve_statement_effects` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src` — used from value-origin stability code in `flow.rs` (read: module `value_origins`).

**Upward trace (read):** `build_state_receiver_reads` sets `StateReceiverReads.complete` from `DirectReads::collect` (`flow.rs:7787-7801`). `call_receiver_read_accesses` (`flow.rs:7628-7684`) uses summarized `footprint.reads` only when `entry.complete` (`flow.rs:7655-7657`); otherwise it widens to the whole receiver. When `complete` stays `true` but `reads` omits places, exclusive borrow pairing (`checks_borrows_calls.rs:764-776`, read) uses the narrowed list.

**Input that reaches the branch:** A callee state body whose receiver operand is a `Name` that does not resolve to a symbol-rooted place via `place_at`, so `place` does not `record` and does not clear `complete`.

**Smallest program (plain words):** A method call whose receiver is an unbound or non-symbol `Name` in a callee state whose footprint is marked complete; borrow checking sees fewer receiver reads than the body performs. Separately, analysis without published flow (`flow: None` at `preserve_statement_effects`) judges value-origin stability only against the statement value-write frame, not operand-specific published writes.

## A0120

**Cited site (read):** `flow.rs:9274-9284` — in `project_constraint_refs_to_active_contexts`, filter predicate keeps `FlowConstraintKind::SemanticContext` only when the context is active; `FlowConstraintKind::Unknown` and `borrow_resource_pattern!(..)` always return `true`.

**Enclosing function (read):** `project_constraint_refs_to_active_contexts` (`flow.rs:9257-9285`).

**Callers (search):** pattern `project_constraint_refs_to_active_contexts` in `omega-rust` — 1 definition (`flow.rs:9257`), 9 production call sites in `flow.rs`, `flow_expression.rs`, `flow_call_phases.rs` (count mode: 11 lines including `use` re-exports and one `#[cfg(test)]` block at `flow.rs:9361`).

**Upward trace (read):** Invoked after call invalidation and state transitions to shrink constraint spans to contexts still active (`flow_call_phases.rs:1308-1313`, read). Borrow-resource and `Unknown` constraints bypass the active-context map at `flow.rs:9282`.

**Input that reaches the branch:** Any call or state merge that runs projection with a constraint span containing `Unknown` or borrow-resource kinds.

**UNDECIDED:** Whether retaining those constraints when sibling semantic contexts were filtered out makes downstream proof goals vacuous, fail-closed, or irrelevant to compilation acceptance was not read in constraint consumers (`flow_expression.rs` evaluation paths only named, not read in full).

## A0121

**Cited sites (read):** `flow.rs:9425-9466` — machines without entry states `continue`; calls with no `call_target_return_type` `continue`. `flow.rs:9468-9482` — ownership events with missing machine or `event_type_reference` `continue`. `flow.rs:9732-9753` — `event_exposure` returns `PrivateImplementation` when root is not a symbol, machine is not public/boundary, or root is not machine/`self`/parameter on entry state.

**Enclosing function (read):** `derive_checked_semantic_dependencies` (`flow.rs:9419-9499+`).

**Callers (search):** pattern `derive_checked_semantic_dependencies` — definition `flow.rs:9419`, re-export `flow.rs:11721`, calls `checking.rs:1183`, `products.rs:70`, `omega/packages/review-evidence/.../semantic_dependencies.rs:32` (read: names only for the last).

**Upward trace (read):** `checking.rs:1183` assigns the derived table into check facts without branching on row count (read: single assignment line). Exposure defaults to private when lookups fail (`flow.rs:9732-9753`).

**Input that reaches the branch:** Typed/fact cross-reference where a machine has no states, a call target has no return type, or an ownership event cannot be typed.

**Closing check:** Omitted rows affect the optional `CheckedSemanticDependencies` product only; no `continue` in this function was read as turning a diagnostic into acceptance in `checking.rs` (read).

## A0122

**Cited sites (read):** `flow.rs:10617-10619` — `disposition.zero_image` yields `Some(Vec::new())`. `flow.rs:10869-10874` — reference/slice types return `Some(())` without descending. `flow.rs:10964-10968` — `DynamicTrait` arm in `visit` returns `Some(())` without leaves (read: `flow.rs:10919-10968` switch). `flow.rs:10759-10767` — dispositions whose machine is not checked-body, open, or non-proof are skipped in dependency build. `flow.rs:10799-10803` — cleanup machine with no entry state `continue`s without error.

**Enclosing functions (read):** `selected_cleanup_leaves_in` (`flow.rs:10600-10684`); `visit` (`flow.rs:10853+`); cleanup dependency loop in same module (`flow.rs:10758-10815`, read).

**Callers (search):** `selected_cleanup_leaves` — `published_facts.rs:896`, `published_facts.rs:1200`, `checks.rs:8881`, `execution_terminal_unit_cleanup.rs:251`, `flow.rs:10791` (read: grep listing).

**Input that reaches the branch:** Contextual cleanup disposition flagged `zero_image`, residual type `DynamicTrait` or reference/slice, or caller machine outside checked-body filter.

**UNDECIDED:** Whether `zero_image` or disposition rows can be recorded for programs that should be rejected was not read in upstream contextual-cleanup publishers (only consumers named).

## A0123

**Cited site (read):** `flow.rs:11511-11516` — when `(value, projection)` is already in `active`, returns `Some(Vec::new())` (cycle).

**Enclosing function (read):** `selected_implementations` (`flow.rs:11500-11621`).

**Callers (search):** `selected_implementations` — definition `flow.rs:11500`, recursive/internal calls `flow.rs:11576`, `flow.rs:11604`, `flow.rs:11665`, `flow.rs:11679`; external entry `flow.rs:11465` inside `dynamic_effect_dependencies::derive` (read).

**Upward trace (read):** `derive` (`flow.rs:11449-11495`) on empty `origins` executes `continue` without diagnostic (`flow.rs:11475-11476`). Published via `published_facts.rs:11208` (read: grep only).

**Input that reaches the branch:** Operand graph where tracing selector implementations revisits the same `(operand, projection)` pair.

**UNDECIDED:** Whether an omitted `CapturedSelectorCall` causes a later pass to accept an under-constrained selector call was not read in `validation/effect_inference.rs` or downstream checks.

## A0124

**Cited site (read):** `flow_call_phases.rs:836-857` — for each ensures contract occurrence, `canonical_place_from_expression` failing returns `true` (survives); non-symbol root returns `true`; `contract_parameter_position` `Ok(None)` returns `true`; only `Ok(Some(position))` consults `observation_survives`.

**Enclosing function (read):** `copied_guarantees::retain_guarantees` (`flow_call_phases.rs:814-893`).

**Callers (search):** pattern `retain_guarantees` in `omega-rust` — 1 definition (`flow_call_phases.rs:814`), 1 call (`flow_call_phases.rs:2284` in `build_call_exit_contexts`).

**Upward trace (read):** `build_call_exit_contexts` (`flow_call_phases.rs:2267-2306`) runs after borrow-call execution; stale ensures mark contexts for removal when any contract fact in a context references a stale proof fact (`flow_call_phases.rs:872-883`, read). Occurrences that always `survives` at `836-855` never push into `stale`.

**Input that reaches the branch:** An `ensures` contract whose referenced occurrences do not map to a callee parameter position (or have no canonical place / non-symbol root).

**Smallest program (plain words):** A call contract `ensures` mentioning a global or expression occurrence that is not a parameter; callee writes invalidate parameter-mapped facts but this filter keeps the ensures proof facts, so exit contexts may still carry the guarantee.

## What I did not read

- Full bodies of constraint evaluation after `project_constraint_refs_to_active_contexts` in `flow_expression.rs` and proof checking of retained contract contexts.
- Publishers of `zero_image` and `ContextualCleanupDisposition` rows beyond the skip sites in `flow.rs:10758-10815`.
- `validation/effect_inference.rs` and corpus fixtures that would show whether empty `CapturedSelectorCalls` rows arise in rejected programs.
- Whether cited line numbers differ at commit `457adf4a11f` (current checkout matched on read; no `git show` run per user no-build rule).

SLICE COMPLETE
