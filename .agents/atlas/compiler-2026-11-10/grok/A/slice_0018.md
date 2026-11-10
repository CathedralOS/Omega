# slice_0018

Area digest preamble (read): `.agents/atlas/compiler-2026-11-10/digests/area_09.md` — stage 05 flow/values/published facts/execution plans under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`; short names FOC, FOW, FP, FRP, FRB, FSV, FT. Citations pinned at `457adf4a11f`; cited excerpts match this checkout (`read`, verified `flow_place.rs:1012` at digest commit via `git show 457adf4a11f:.../flow_place.rs`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0133 | flow_operand_contexts.rs:111-114 | REACHABLE | `CaseEvidence::prepare` `None` makes `path_is_inactive` false, so case-tagged paths are treated as active when evidence is missing. |
| A0134 | flow_ownership.rs:67-69 | REACHABLE | Unresolved callee parameters or place-like expressions without an ownership place record no move events during discovery. |
| A0135 | flow_place.rs:1012 | REACHABLE | Case copy and stable-subject logic do not verify mutability below an immutable root; any valid expression root counts as stable. |
| A0136 | flow_reference_places.rs:526-531 | REACHABLE | Symbol-rooted storage not explained as local or parameter is returned as its own storage without rebasing. |
| A0137 | flow_retained_borrows.rs:1368-1391 | REACHABLE | Retained borrows that fail the “owing” loan predicate still pass the handback filter and are handed back. |
| A0138 | flow_state_values.rs:518-541 | REACHABLE | Skipped lexical-region delivery never calls `join`, so entry inputs stay at the initial unknown row instead of call-specific facts. |
| A0139 | flow_transfers.rs:3599-3602 | REACHABLE | Empty `observation_places` allows scalar values to survive a Unit call despite callee storage writes outside that list. |
| A0140 | flow_transfers.rs:1085-1115 | REACHABLE | Non-exact assignment targets keep domain memberships when any one source context proves them, not when all do. |

## A0133

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow_operand_contexts.rs:106-114` — `OperandContextSelection::path_is_inactive` is true only when the subject has a case segment and `self.cases().as_ref().is_some_and(|cases| cases.path_is_inactive(subject))`.

**Enclosing type methods (read):** `cases()` (`flow_operand_contexts.rs:82-93`) initializes `CaseEvidence::prepare(...)` into `OnceCell<Option<CaseEvidence>>`. `prepare` (`flow_place.rs:511-521`) returns `None` when `machine_by_symbol` or `state_in_machine` fails.

**Parallel site (read):** `place_case_path_is_inactive` (`flow_place.rs:1181-1205`) uses the same `prepare(...).is_some_and(|evidence| evidence.path_is_inactive(subject))` shape; when `prepare` is `None`, the function is `false`.

**Callers (search):** pattern `\bpath_is_inactive\b` in `omega-rust` — 9 matches in 5 files: definitions `flow_operand_contexts.rs` (2), `flow_place.rs` (2); uses `flow_transfers.rs:1830`, `checks_contracts.rs:1710` (`selection.path_is_inactive`), `checks_contracts_exits.rs:4586`.

**Upward trace (read):** `OperandContextSelections::for_operand` builds `OperandContextSelection` for contract/borrow operand proof (`checks_contracts.rs:1589-1806`, read call sites only). Operand nominal-field satisfaction combines `cases_are_selected` with `path_is_inactive` (`checks_contracts.rs:1707-1710`). When `prepare` is `None`, `cases_are_selected` is also false for case-tagged subjects (`flow_operand_contexts.rs:95-103`), but any site that treats “not inactive” as permission without a paired `cases_are_selected` gate will behave as if the case path were active.

**Input:** Operand projection on a case-tagged place at a statement whose machine or state symbol does not resolve in `CheckingInput`, so `CaseEvidence::prepare` returns `None`.

**Smallest program (plain words):** A call operand proof on a case-projected value where case evidence cannot be prepared; the flow layer reports the path as not inactive, so inactive-case discharge never runs and checking can accept obligations that should be refused.

## A0134

**Cited sites (read):** `flow_ownership.rs:67-69` — `call_target_parameters` `None` makes `append_call_ownership_events` return without recording moves. `flow_ownership.rs:862-910` — place-like expression with `stored_value_requires_transfer` but `ownership_place_from_expression_in_state` `None` hits `return` at `910` without move or observation.

**Enclosing functions (read):** `append_call_ownership_events` (`flow_ownership.rs:36-165`); `moves::append_move_events_for_expression` (`flow_ownership.rs:779-1018`).

**Callers (search):** pattern `\bappend_call_ownership_events\b` in `omega-rust` — 4 matches, all in `flow_ownership.rs` (definition plus comment references; production call `flow_ownership.rs:1984` inside `discover_state_move_events`).

**Upward trace (read):** `discover_state_move_events` (`flow_ownership.rs:1945-1988`) walks each statement’s borrow calls and accumulates move events. `checks_multiplicity_permission_events.rs:835-842` consumes `discover_state_move_events` for permission/move checking.

**Input:** (1) Borrow call row whose target has no resolvable parameter list. (2) Call argument expression classified place-like needing transfer but not mapped to an ownership place in the caller state.

**Smallest program (plain words):** A call that should move an affine owned local is emitted with an unresolved callee signature or a place-like argument with no ownership place; discovery records no move, and multiplicity checking sees no transfer obligation.

## A0135

**Cited sites (read):** `flow_place.rs:1012-1069` — `dispatch_copy_carries_case` treats a copy as carrying the case when `place_root_is_immutable(&source)` (`1068`) after matching an immutable local initializer; it does not check mutability along segments below that root. `flow_place.rs:1239-1241` — `stable_subject` treats `PlaceRoot::Expression(expression)` as stable when `expression.is_valid()` without further custody checks.

**Enclosing functions (read):** `dispatch_copy_carries_case` (method on `CaseEvidence`, `flow_place.rs:1012-1070`); `stable_subject` (`flow_place.rs:1231-1264`).

**Callers (search):** `dispatch_copy_carries_case` — 2 matches in `flow_place.rs` (definition, call `752` inside `has_value`). `stable_subject` / `stable_case_subject` — 18 matches across `flow_place.rs`, `flow_observations.rs`, `flow_exits.rs`, `checks_multiplicity_cleanup_dispositions.rs`, `checks_ranges.rs` (read: names only).

**Upward trace (read):** `has_value` (`flow_place.rs:734-767`) uses `stable_subject` and may set `selected` via `dispatch_copy_carries_case`. `CaseEvidence::path_is_inactive` / `cases_are_selected` and observation visitors (`flow_observations.rs:766`, `1605`, read) gate case facts on `stable_case_subject`.

**Input:** Owned case copy from an immutable-rooted source where nested storage under the root could still be mutated, or expression-rooted subject handle marked valid without stable storage.

**Smallest program (plain words):** A transition proves a case tag via a copy from an immutable binding while a nested field beneath that root remains mutable; the compiler accepts the case as carried and treats the subject as stable.

## A0136

**Cited sites (read):** `flow_reference_places.rs:526-531` — after parameter/local rebasing fails, if the place is symbol-rooted and not a readable reference, `storage_places_for_canonical_place` returns `Some(vec![place.clone()])`. `flow_reference_places.rs:858-860` — `local_reference_storage_before_statement` returns `Some(place)` unchanged when no matching `LocalData` declaration exists for the root symbol.

**Enclosing functions (read):** `storage_places_for_canonical_place` (`flow_reference_places.rs:220-532`); `local_reference_storage_before_statement` (`flow_reference_places.rs:834-869`).

**Callers (search):** `argument_storage_places` — 4 matches (`flow_reference_places.rs` definition/export, `flow_state_values.rs:2516`, `flow.rs:11700`). `storage_places_for_canonical_place` — 12 call sites, all within `flow_reference_places.rs` (`read`).

**Upward trace (read):** `argument_storage_places` feeds flow state value capture (`flow_state_values.rs:2516`, read snippet). Storage resolution backs transfer and borrow footprint planning that assumes referents are real storage.

**Input:** Canonical place rooted at a state symbol that is neither an in-scope parameter/local nor a readable reference binding, passed into argument storage resolution.

**Smallest program (plain words):** A call or transition names storage at a bare symbol root that does not correspond to a declared local or parameter; the compiler accepts that symbol as the storage site instead of refusing or rebasing.

## A0137

**Cited sites (read):** `flow_retained_borrows.rs:1271-1277` — `handbacks` returns empty when `argument_sources` or `ending_inputs` is missing. `flow_retained_borrows.rs:1358-1393` — returned borrows are kept when the `state_loans` predicate at `1368-1391` does not match a lifetime-dependency loan (`filter` keeps rows where the inner `any` is false).

**Enclosing function (read):** `handbacks` (`flow_retained_borrows.rs:1262-1394`).

**Callers (search):** pattern `\bhandbacks\b` in stage-05 — 3 matches: definition `flow_retained_borrows.rs:1262`, calls `flow_retained_borrows.rs:213`, `flow_call_phases.rs:1875`.

**Upward trace (read):** `flow_call_phases.rs:1875-1897` (read) emits domain membership rows from each handback. Missing sources yield no handbacks (safe); retained `ReturnedBorrow` rows that are not classified as “owing” under the lifetime filter still become handbacks and downstream memberships.

**Input:** Call completion with `ArgumentSources` present, but a retained borrow returned from the callee does not match the lifetime-dependency overlap test on existing loans.

**Smallest program (plain words):** A call returns a retained borrow that the loan ledger does not recognize as still owing; handback construction keeps it and call-phase code publishes memberships that should be rejected.

## A0138

**Cited sites (read):** `flow_state_values.rs:506-541` — `lexical_regions::record` returns early on reachability, target resolution, visibility, entry-state, or static-input guards without calling `join`. Comment at `flow_state_values.rs:2763-2765` (read): “A missing row is unreachable.” Successful path calls `join` at `674-692` (`read`).

**Enclosing function (read):** `lexical_regions::record` (`flow_state_values.rs:496-693`).

**Callers (search):** `lexical_regions::record` — invoked from `flow_calls.rs:72-81` during call fact recording (`read`).

**Upward trace (read):** Initial `unknown_inputs` seeds every state row (`flow.rs:519`, `flow_state_values.rs:2962-2979`, read). `join` (`flow_state_values.rs:2844-2958`) creates or merges `build.state_value_inputs`. `append_entry_context` (`flow_state_values.rs:3499-3544`) publishes scalar transfer facts only when `reached_state_index` finds a row; skipped `record` leaves parameters at `ScalarValue::Unknown` without call-specific meet evidence.

**Input:** Nested or closed-private lexical-region call that fails one of the guards at `518-541` (not nested/extracted, wrong entry, static bounded inputs, failed authentication) while the callee entry is still reached through other flow.

**Smallest program (plain words):** An extracted-region call that should deliver caller roster scalars into the private entry but fails the static-input or authentication guard; compilation keeps unknown entry parameters instead of rejecting the call, and later proofs may treat unknown as sufficient.

## A0139

**Cited sites (read):** `flow_transfers.rs:3599-3602` — `retains_values_across_unit_call` requires `call_storage_writes(...)? .observation_places.is_empty()`. Comment at `flow_transfers.rs:2275-2278` (read) speaks of “complete storage footprint is empty” while the code inspects `observation_places` only.

**Enclosing function (read):** `retains_values_across_unit_call` (`flow_transfers.rs:3510-3603`); called from `normal_returns::capture` statement loop (`flow_transfers.rs:1982-1994`, read).

**Callers (search):** pattern `\bretains_values_across_unit_call\b` — 3 matches in `flow_transfers.rs` (definition, import, call).

**Upward trace (read):** `call_storage_writes` (`flow_call_phases.rs:1095-1154`, read) sets `observation_places` from mutation observation list, distinct from full physical write footprint (`binding_places`). When the test passes, `capture_call` / normal-return scalar capture keeps caller `CallValues` across the Unit callee (`flow_transfers.rs:2280-2476`, read structure).

**Input:** Checked-body Unit call where the callee performs storage writes not listed in `observation_places` (writes present in contributions but outside the observation vector).

**Smallest program (plain words):** A statement sequence binds scalars, calls a Unit-returning machine that mutates storage not recorded as an observation place, and the capture pass treats the call as footprint-empty and keeps pre-call scalar facts.

## A0140

**Cited sites (read):** `flow_transfers.rs:1085-1115` — for each `candidate` in `candidate_targets`, `declared_domains::establish` appends a membership fact when `was_live`: `assignment_source_contexts` contexts are scanned with `.iter().any(|reference| context_view(...).proves_place_domain_membership_in_program(...))` (`1092-1105`). Non-exact path is taken when `!write.target_is_exact` (`flow_transfers.rs:3974-3988`, read).

**Enclosing function (read):** `declared_domains::establish` (`flow_transfers.rs:919-1116`).

**Callers (search):** `declared_domains::establish` — 2 matches in `flow_transfers.rs` (`3975`, `4168`).

**Upward trace (read):** Assignment storage write handling (`flow_transfers.rs:3960-3988`, read) invokes `establish` for inexact targets with `candidate_targets` built from indexed/fixed-index projections (`1039-1083`, read). Membership facts feed the semantic plan consumed by contract and range checkers via `FactPlan`.

**Input:** Assignment through a reference local whose referent is ambiguous (multiple `candidate_targets`), domain membership proved in only one predecessor context but not all contexts that could reach the write.

**Smallest program (plain words):** A write targets a reference local that may point to several storages; one context still proves a field domain membership, and the transfer keeps that membership on the candidate even though another feasible referent does not satisfy the domain.

## What I did not read

- Full bodies of `call_target_parameters`, `ending_inputs`, `call_storage_mutations`, `call_mutation_writes`, and `MembershipEvidence::prepare` beyond call sites.
- Stage-06 lowering or `omega run` behavior that would prove whether skipped lexical delivery or missing moves arise only on malformed inputs.
- Entirety of `checks_contracts_prover.rs` and `checks_multiplicity.rs` beyond the traced call edges.

SLICE COMPLETE
