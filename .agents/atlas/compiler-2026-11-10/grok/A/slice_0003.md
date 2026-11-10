# slice_0003

Area digest preamble (read): stage 05 builds the borrow ledger and runs fixed checks (`borrow.rs`, `checks.rs`, `checks_borrows*.rs`, contracts, multiplicity, crashes). Words: open / stated / traced / suspected / inferred. Citations pinned at `457adf4a11f`; current checkout matches cited excerpts (verified with `git show 457adf4a11f:.../checks.rs` for each row’s lines).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0017 | checks.rs:5750-5753 | REACHABLE | Missing `statement_at` yields “not used after call,” so live-value carry demand can be skipped for a may-suspend call. |
| A0018 | checks.rs:6972-6977 | REACHABLE | `transport_suppliers` match arm is empty; lifetime join can succeed without DirectRoot/Reborrow/SuppliedOutput validation. |
| A0019 | checks.rs:6299-6301 | REACHABLE | Empty `borrow_carrying_owner_paths` forces `ViewFree`, then `retained_lifetime_demand` returns permissive carry. |
| A0020 | checks.rs:7448-7457 | REACHABLE | `continue` on missing machine/state/flow/call_flow skips suspension-carry work for that borrow call without a diagnostic. |
| A0021 | checks.rs:7289-7291 | REACHABLE | No matching carry-claim origins → `structural` policy from type requirements, possibly looser than the permission claim. |
| A0022 | checks.rs:8633-8639 | REACHABLE | Saved-subject owned elsewhere → `expression` returns with no steps, so ownership occurrence order can omit that subtree. |
| A0023 | checks.rs:8959-8961 | REACHABLE | `TransitionNamed` calls and empty `machine_states` make `calls_complete` vacuously true inside cleanup admission. |
| A0024 | checks.rs:10374-10381 | REACHABLE | Machines without `contract_plans` rows and constant entries missing crash summaries are skipped, not rejected. |

## A0017

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checks.rs:5750-5753` — in `intra_statement::place_is_used_after_call`, if `program.statement_table.statement_at(state, statement_index)` is `None`, the function returns `false` (not “used after target”).

**Enclosing function (read):** `place_is_used_after_call` (`checks.rs:5742-5766`) builds `EvaluationTraversal` and visits the statement; `observe_place` (`checks.rs:5936-5955`) sets `place_used_after_target` only after `target_reached`.

**Callers (search):** pattern `place_is_used_after_call` in `omega-rust` — 1 definition (`checks.rs:5742`), 1 production call (`checks.rs:7804` inside `append_live_value_carry_demands`); 0 other matches.

**Upward trace (read):** `append_live_value_carry_demands` (`checks.rs:7780-7834`) adds a symbol to `live_symbols` only when `place_is_used_after_statement` **or** `place_is_used_after_call` is true (`checks.rs:7797-7812`). Values not in `live_symbols` are skipped for carry obligations (`checks.rs:7817-7822`). Caller: `check_suspension_carry` (`checks.rs:7529-7540`). Diagnostics come from `resolve_crossing_carry` (`checks.rs:7896-7973`) via `finish` (`checks.rs:12230-12235`). `check_checked_facts_recording` appends carry errors (`checks.rs:12156-12159`).

**Input that reaches the branch:** A may-suspend borrow-call row whose `statement_index` has no `statement_at` entry while a parameter/local would still be live after that call site in the real program.

**Smallest program (plain words):** A machine whose borrow ledger records a suspending call at a statement ordinal that does not exist in the typed statement table, while a suspension-forbidden value would remain live after that call; checking treats the place as unused-after-call and may accept.

## A0018

**Cited site (read):** `checks.rs:6972-6977` — in `loans::retained_lifetime_demand`, after other `CheckedParentBorrowResource` arms, `_ if transport_suppliers(...).is_some() => {}` performs no validation.

**Enclosing function (read):** `retained_lifetime_demand` (`checks.rs:6697-7002`) walks loan links from `value.borrow_frontier`; success returns `Ok(CarryPolicy { suspension: Allowed, ..STRICT })` (`checks.rs:6998-7001`).

**`transport_suppliers` (read):** `checks_borrows.rs:7922-7940` — `Some` only for `UnretainedDerived` loans with non-empty, state-owned suppliers and matching `direct_reborrow_effect`.

**Callers of `retained_lifetime_demand` (search):** pattern `retained_lifetime_demand` in `omega-rust` — 1 definition (`checks.rs:6697`), 1 call (`checks.rs:7912` in `resolve_crossing_carry`).

**Callers of `transport_suppliers` (search):** `checks.rs:6972`, `checks_borrows.rs:8128`, `checks_borrows_resources.rs:1756`, `checks_borrows_resources.rs:1820` (read: names only).

**Upward trace (read):** `resolve_crossing_carry` (`checks.rs:7908-7973`) maps lifetime `Ok(demand)` into policy; `Allowed` suspension skips diagnostic (`checks.rs:7946-7948`). `check_suspension_carry` uses `finish(diagnostics)` (`checks.rs:7586`).

**Input:** A crossing live value whose retained-loan row hits the supplier arm without a validated DirectRoot/Reborrow/SuppliedOutput resource binding.

**Smallest program (plain words):** A suspending call while a borrowed value’s lifetime walk encounters only a descriptor-transfer supplier edge and no three-way resource kind; the empty arm lets the join complete as suspension-allowed.

## A0019

**Cited site (read):** `checks.rs:6299-6301` — `value_frontier` returns `SuspensionValueBorrowFrontier::ViewFree` when `paths.is_empty()` after `borrow_carrying_owner_paths` (`checks.rs:6294-6300`).

**Enclosing function (read):** `value_frontier` (`checks.rs:6267-6384`); non-empty paths require matching retained loans or `Unresolved`.

**Callers (search):** `value_frontier` in `checks.rs` — definition `6267`, sole call `7542` inside `check_suspension_carry`.

**Upward trace (read):** `retained_lifetime_demand` (`checks.rs:6709-6710`) — `ViewFree` ⇒ `Ok(CarryPolicy::PERMISSIVE)` immediately. `resolve_crossing_carry` may allow suspension without error when policy allows (`checks.rs:7946-7948`).

**Input:** Type reference whose `borrow_carrying_owner_paths` is empty (or reports no paths) at a crossing value that still has active retained loans in the frontier arena.

**Smallest program (plain words):** A value whose type metadata carries no borrow paths while suspension retention still holds loan facts; the checker labels the frontier view-free and applies permissive carry.

## A0020

**Cited sites (read):** `checks.rs:7448-7457` and related skips in `check_suspension_carry`: missing machine (`7448-7450`), missing state (`7451-7453`), missing `state_flow` (`7455-7457`); missing `call_flow` (`7460-7468` `continue`); `collect_call_carried_values` early return when `call_site` is `None` (`checks.rs:7708-7710`, read).

**Enclosing function (read):** `check_suspension_carry` (`checks.rs:7434-7588`) loops borrow states and calls; only matching suspension calls get `resolve_crossing_carry`; result `finish(diagnostics)` (`7586`).

**Callers (search):** `check_suspension_carry` — called from `check_checked_facts_recording` (`checks.rs:12156-12159`, read).

**Input:** `BorrowCallFact` / flow rows that imply `direct_may_suspend` or `transitive_may_suspend` but lack a matching `FlowCallFact`, or state/machine symbols absent from the typed program or flow index.

**Smallest program (plain words):** A may-suspend call present in borrow facts with no matching flow call row (or with a machine/state symbol the typed tree cannot resolve); the loop `continue`s and never emits a suspension-carry diagnostic for that call.

## A0021

**Cited site (read):** `checks.rs:7289-7291` — `ClaimCarryContext::effective_policy` returns `structural` when `origins.is_empty()` after scanning entry contexts (`7253-7287`).

**Enclosing function (read):** `effective_policy` (`checks.rs:7229-7298`); comment at `7222-7227` (read) states no claim entry follows structural/type-wide policy.

**Callers (search):** `effective_policy(` in stage-05 — calls at `checks.rs:7872`, `checks.rs:7876` inside `collect_value_carry_obligation` (`7858-7894`).

**Upward trace (read):** `collect_call_carried_values` passes `program.expression_table.display_name(*argument)` as `value_name` (`checks.rs:7726-7732`); `effective_policy` matches `place_label(program, place)` to `value_name` (`checks.rs:7267-7268`). Mismatch leaves `origins` empty → structural. `check_suspension_carry` consumes policies in `resolve_crossing_carry`.

**Input:** Call argument whose display name does not equal the semantic place label used on permission carry-claim facts in entry contexts.

**Smallest program (plain words):** A suspending call with an argument whose printed name differs from the claim’s place label so carry claims are ignored and the type’s structural carry policy (often more permissive) governs.

## A0022

**Cited site (read):** `checks.rs:8633-8639` — `StatementOrder::expression` calls `saved_dispatch_subject`; if the subject belongs to another statement or expression handle, `return` without pushing steps.

**Enclosing function (read):** `StatementOrder::expression` (`checks.rs:8618-8700+`); `statement_steps_at_evaluation` (`checks.rs:8306-8444`) drives ordering.

**Callers of ordering (search, read names):** `statement_steps_at_evaluation` used from `checks_multiplicity.rs`, `checks_borrows.rs`, `checks_borrows_calls.rs`, and internally `statement_steps` (`checks.rs:8251`); production multiplicity/borrow checks consume occurrences.

**Upward trace (read):** Fewer `StatementOccurrence` rows ⇒ fewer `Observe`/`Move` steps for downstream permission and borrow-order checks; no alternate diagnostic at this `return`.

**Input:** Expression node evaluated in statement *S* whose saved-dispatch subject was recorded for a different statement or a different expression handle.

**Smallest program (plain words):** A statement reuses a saved-subject expression bound elsewhere; ownership occurrence inventory omits that subtree and may accept an ordering that should be rejected.

## A0023

**Cited sites (read):** `checks.rs:8959-8961` — in `cleanup_admission::validate_selected_cleanup_guarantees`, `calls_complete` inner closure: `CallSite::TransitionNamed { .. }` ⇒ `return true` without `crash.checked_call_at`. `checks.rs:8940-8946` — `.all(...)` over `facts.flow.control.machine_states(target)`; empty iterator ⇒ vacuous `true` (read).

**Enclosing function (read):** `validate_selected_cleanup_guarantees` (`checks.rs:8860-9040`); returns `finish(diagnostics)` (`9040`). Caller `checking.rs:1156` (read) — `cleanup?` rejects compilation when `Err`.

**Input:** Selected linear cleanup target with named-transition call sites inside the cleanup machine body, and/or a cleanup machine with no flow states in `facts.flow.control`.

**Smallest program (plain words):** An automatically selected cleanup machine whose checked crash contract is not exercised for named-transition calls (or has no flow states to iterate); admission treats call coverage as complete and may accept the cleanup.

## A0024

**Cited sites (read):** `checks.rs:10374-10381` — `infer_path_conditioned_guard_coverage` `continue`s when no `contract_plans` row for `machine.symbol`. `checks.rs:10798-10800` — `check_constant_expression_entries_cannot_crash` `continue`s when `infer_checked_crash_causes` has no summary for `entry`.

**Enclosing functions (read):** `infer_path_conditioned_guard_coverage` (`10363+`); `check_constant_expression_entries_cannot_crash` (`10771-10816`). Invoked from `check_checked_facts_recording` (`12124-12141`, read): coverage diagnostics are extended; constant check `Err` only when diagnostics non-empty (`10812-10815`).

**Input (path-conditioned):** `program.machines()` includes a machine with no matching `facts.contract_plans.machines()` entry while other machines still get crash refinement.

**Input (constant entry):** Constant recipe lists an `evaluation_entry` absent from `summaries` while the expression still has crash causes.

**Smallest program (plain words):** (1) A program machine lacking a contract-plan row skips path-conditioned crash refinement without failing the pass; (2) a build-time constant entry omitted from inferred crash summaries is not checked for crash causes and may be accepted.

## What I did not read

- Full bodies of `borrow::view_link::borrow_carrying_owner_paths`, `validation::find_call_site`, and `crate::published::facts::infer_checked_crash_causes` (only call sites and slice-local control flow).
- Entirety of `checks_borrows.rs` / `checks_borrows_resources.rs` beyond `transport_suppliers` and caller line numbers.
- Stage-04/fact-build code that would prove whether desynchronized borrow rows, missing `contract_plans` rows, or missing crash summaries arise in normal compilation (would decide if some branches are production-reachable vs fixture-only).

SLICE COMPLETE
