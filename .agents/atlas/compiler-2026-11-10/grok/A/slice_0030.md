# slice_0030

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0224 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/projections.rs:1835-1866 | REACHABLE | Task-frame lowering records bad call targets as `unresolved_calls` and returns `Ok`; activation lowering installs the graph without rejecting them. |
| A0225 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:2625-2626 | REACHABLE | Signed literal `-1` satisfies `*value != 0` in `safe_policy_structural_divisor`, so policy division lowering skips nonzero-divisor evidence. |
| A0226 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:298-308,368-370 | REACHABLE | Empty resolved content or projection list uses `continue`, dropping the entry claim while `lower_whole_content_entry_claims` still succeeds. |
| A0227 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:4452-4463 | REACHABLE | All-`false` guards normalize away and `CrashRouteBucket::new` returns `None` for empty guards, so the bucket is dropped without error. |
| A0228 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:4220,4326 | UNDECIDED | Default match arms skip substitution; I did not enumerate which `ScalarTerm` / `Proposition` kinds can appear in lowered crash buckets that still need rebasing. |
| A0229 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:4751-4766 | UNDECIDED | Omitted applications error at evidence lookup (5050) and proof-output lookup (6543); other consumers of the lowered application list were not traced. |
| A0230 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:7186-7256 | REACHABLE | Each `Direct*` source arm falls back to `transitional_input` when `direct_source` is `None`, so projection lowering succeeds without a resolved binding. |
| A0231 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:7319-7324,7400-7402,7417,7449 | REACHABLE | These `Ok(None)` returns from `resolve_direct_float_source_binding` feed entry 55 (`lower_float_meaning_projection` transitional fallback). |

## A0224

**Digest:** area_10 record 068 item 49 (`projections.rs:1835-1866`).

**Site (read):** In `task_frame_summary` (`projections.rs:1721-1887`), the flow-call loop (`1812-1870`) calls `checked_call_target` (`1926-1946`). When it returns `Ok(None)` (`1937-1938`), the code pushes `TerminalTaskUnresolvedCall` with `UnresolvedTarget` (`1838-1849`) and `continue`s. When the target resolves but `callee_machine.supply_mode != MachineSupplyMode::CheckedBody` (`1852`), it pushes `NonCheckedSupply` (`1853-1864`) and `continue`s. The frame is returned in `Ok(TaskFrame { …, unresolved_calls, … })` (`1873-1886`).

**Callers (read):** `rg '\btask_frame_summary\b' omega-rust` → 2 matches in `projections.rs` (definition `1721`, call `1678`). `rg '\btask_call_graph\b' omega-rust` → 2 matches in `projections.rs` (definition `1651`, call `2306`). Sole production caller of `task_call_graph` is `lower_task_activation_demands` (`projections.rs:2305-2306`). `rg '\blower_task_activation_demands\b' omega-rust` → 8 matches: production install at `machine_lowering.rs:1794-1796`; re-exports/tests elsewhere.

**Upward trace (read):** `lower_terminal_selection` retention assigns `lowered.semantic_module.task_activation_demands = crate::lower_task_activation_demands(…)?` (`machine_lowering.rs:1794-1796`). Errors from `task_frame_summary` only arise from other branches (suspension crossing, coordinate overflow, etc.), not from unresolved targets. Stage 05: `rg 'NonCheckedSupply|UnresolvedTarget|unresolved.*call' omega-rust/psi/**/05**` → no matches (read).

**Consumers (read):** `foundation_validation.rs:9387-9398` validates uniqueness/shape of existing `unresolved_calls` rows; it does not treat them as a lowering failure. `omega/compilation/backend/runtime/task-plans/src/activation.rs:378-411` copies them into stack summaries.

**Input (read):** A checked program whose task WCSU call graph includes a flow call at a statement that names no typed state, or names a machine state whose supply mode is not `CheckedBody`, after prior stages have already accepted that checked graph.

**Verdict:** REACHABLE — lowering accepts the program and records the call instead of failing in this loop.

## A0225

**Digest:** area_10 record 069 item 50 (`proofs.rs:2625-2626`).

**Site (read):** `safe_policy_structural_divisor` (`proofs.rs:2613-2645`) matches `ScalarTerm::Integer { …, IntegerValue::Signed(value) }` and returns `*scalar_type == integer_type && *value != 0` (`2623-2626`), so signed literal `-1` is accepted as safe.

**Callers (read):** `rg '\bsafe_policy_structural_divisor\b' omega-rust` → 3 matches in `proofs.rs` (definition `2613`, re-export `7751`, use `7983`).

**Upward trace (read):** In structural integer binary lowering (`7922-7987`), wrapping/saturating divide/remainder with `!*formation_decided` requires `safe_policy_structural_divisor(…)` (`7983`); failure is `unsupported("structural crash policy division requires explicit terminal nonzero-divisor evidence")` (`7985-7987`). Literal `-1` passes the guard at `2625-2626` without `runtime_requirements`.

**Input (read):** A checked structural crash predicate whose policy divide/remainder uses a signed integer literal `-1` as divisor and has not had formation decided, so the policy path runs at `7983`.

**Verdict:** REACHABLE — program lowering accepts the divide without nonzero-divisor evidence because `-1` is treated as safe.

## A0226

**Digest:** area_10 record 069 item 51 (`proofs.rs:298-308`, `368-370`).

**Site (read):** `lower_whole_content_entry_claims` (`proofs.rs:237-385`) iterates `entry_claims`. Qualifications are resolved via `for_semantic_domain` inside `filter_map` (`298-304`); if `content.is_empty()`, `continue` (`306-307`). After mapping projections, `if projections.is_empty() { continue }` (`368-369`).

**Callers (read):** `rg '\blower_whole_content_entry_claims\b' omega-rust` → 3 matches: definition `proofs.rs:237`, `attached_unit.rs:10560`, `unit_composed_control.rs:10401`.

**Upward trace (read):** Callers treat `Result<Vec<ContentEntryClaim>, LoweringError>` as success with a shorter vector; dropped rows do not error. I did not read a stage-05 rejection that fires on the same “no content qualification” shape (not in call-chain read).

**Input (read):** A checked unit with an `CheckedUnitEntryClaimPlan` row whose parameter qualifications (whole or projected) do not resolve to any content projection in `checked.facts.qualifications.content`, or resolve but yield no valid projections after filtering.

**Verdict:** REACHABLE — program completes lowering without the entry claim rather than being rejected here.

## A0227

**Digest:** area_10 record 069 item 52 (`proofs.rs:4452-4463`; reader did not read `CrashRouteBucket::new`).

**Site (read):** `normalize_formal_crash_routes` (`4440-4465`) `filter_map`s buckets, folding `CrashRouteGuard::Predicate` with constant `false` to `None` (`4451-4452`), `true` to `Truth` (`4453-4454`), then calls `checked_trees::CrashRouteBucket::new(bucket.cause(), guards)` (`4462`).

**Closing check (read):** `CrashRouteBucket::new` (`contract_and_obligation_facts.rs:1401-1414`) returns `None` when `alternative_guards` is empty after normalization (`1411-1414`); `filter_map` drops that bucket.

**Callers (read):** `rg '\bnormalize_formal_crash_routes\b' omega-rust` → 3 matches, all in `proofs.rs` (`4404`, `4419`, definition `4440`). Used from `lower_boundary_crash_routes` (`4378-4409`) and `lower_formal_crash_routes` (`4412-4420`). `lower_boundary_crash_routes` called from `attached_unit.rs:327`; `lower_formal_crash_routes` from `retention.rs:2955`.

**Upward trace (read):** Lowering proceeds with fewer crash route buckets; no error when normalization removes a bucket entirely.

**Input (read):** Checked crash route bucket whose predicate guards are all constant `false` (or otherwise normalize to an empty guard list).

**Verdict:** REACHABLE — bucket is silently omitted instead of rejecting the program.

## A0228

**Digest:** area_10 record 069 item 53 (`proofs.rs:4220`, `4326`).

**Site (read):** Inside `substitute_structural_crash_route_roots` (`4163-4375`), nested `substitute_term` (`4172-4223`) ends with `_ => {}` (`4220`). Nested `substitute_proposition` (`4225-4333`) handles listed proposition shapes; `Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}` (`4326`).

**Callers (read):** `rg '\bsubstitute_structural_crash_route_roots\b' omega-rust` → 5 matches: definition/re-export in `proofs.rs`, import/use in `unit.rs:63`, `attached_unit.rs:12902` and `13337`.

**Upward trace (read):** `attached_unit.rs:13337` invokes substitution over lowered crash continuations during unit lowering. I did not read the full set of `ScalarTerm` / `Proposition` variants that can appear in those buckets after `lower_structural_crash_predicates` / `lower_boundary_crash_routes`.

**Verdict:** UNDECIDED — whether an unsubstituted term or proposition can leave a wrong place id in a guard that should have blocked acceptance was not decided without that variant census and a witness path.

## A0229

**Digest:** area_10 record 069 item 54 (`proofs.rs:4751-4766`; traced only at `5050` and `6543` per digest).

**Site (read):** In `lower_proposition_vocabulary` (`4652-4799`), the application loop (`4743-4793`) does `let Some(declaration) = declaration_ids.find_map(…)` else `continue` (`4751-4755`), omitting applications whose `application.declaration` is absent from lowered declarations.

**Callers (read):** `rg '\blower_proposition_vocabulary\b' omega-rust` → 3 matches in `proofs.rs` (`4652`, `6661`, call `6691` inside `lower_and_install_evidence_artifacts`).

**Traced consumers (read):** `lower_evidence_terms` (`4988+`) requires declaration (`5005-5011`) and terminal application id (`5042-5051` → error `"checked evidence term has no terminal proposition application"`). `terminal_proposition_application_id` (`6473+`) errors at `6543-6544` when no matching lowered application.

**Upward trace (read):** If a checked application references a declaration symbol not in `declaration_ids`, it is omitted at `4755`; whether compilation still succeeds depends on whether any later pass requires that application id. Evidence and proof-output paths fail when they need a missing application (read). Other readers of `applications` after `6691` were not fully traced.

**Verdict:** UNDECIDED — acceptance impact for applications omitted but not referenced by the traced lookups (and any untraced consumers) was not determined.

## A0230

**Digest:** area_10 record 069 item 55 (`proofs.rs:7186-7256`).

**Site (read):** `lower_float_meaning_projection` (`7163-7291`) maps each `CheckedFloatProjectionSource::Direct*` arm: when `direct_source` is `None`, it calls `transitional_input(…)` (`7186`, `7200`, `7214`, `7228`, `7242`, `7256`).

**Callers (read):** `rg '\blower_float_meaning_projection\b' omega-rust` → used from `machine_lowering.rs:1117` inside `retain_float_meanings` (`1094-1120`). `rg '\bretain_float_meanings\b' omega-rust` → 3 matches in `machine_lowering.rs`.

**Upward trace (read):** `retain_float_meanings` passes `direct_source` from `resolve_direct_float_source_binding(…)?` (`1107-1116`) into `lower_float_meaning_projection`; `None` triggers transitional fallback rather than error in the projection function.

**Input (read):** Checked float meaning projection whose source is a `Direct*` variant but resolution yields `None` (see A0231).

**Verdict:** REACHABLE — projection lowering succeeds with `TransitionalInput` instead of rejecting.

## A0231

**Digest:** area_10 record 069 item 56 (`proofs.rs:7319-7324`, `7400-7402`, `7417`, `7449`).

**Site (read):** `resolve_direct_float_source_binding` (`7294-7450`): non-direct source kinds → `Ok(None)` (`7317`). Missing `machine_bindings` owner → `Ok(None)` (`7319-7323`). `DirectCallResult`: no matching call occurrence → `Ok(None)` (`7400-7401`). `DirectOperationResult` → `Ok(None)` (`7417`). Default arm `_ => Ok(None)` (`7449`).

**Callers (read):** 2 matches: definition `7294`, call `machine_lowering.rs:1107` (same path as A0230).

**Upward trace (read):** These `Ok(None)` results are not errors in `retain_float_meanings`; they feed `lower_float_meaning_projection` transitional branches (`7186-7256`, read).

**Input (read):** Examples: projection owner machine not in `projection_sources` mapping (`7319-7323`); float projection tied to a call site with no matching lowered occurrence (`7400-7401`); `DirectOperationResult` source (`7417`); or other kinds hitting `_ => Ok(None)` (`7449`).

**Verdict:** REACHABLE — same acceptance outcome as A0230 via transitional fallback.

## What I did not read

- Stage 05 sources for task-call target validation beyond the empty `rg` under `omega-rust/psi/**/05**` (A0224).
- Full inventory of `ScalarTerm` / `Proposition` variants present in lowered crash buckets at `substitute_structural_crash_route_roots` time (A0228).
- All consumers of the `applications` vector from `lower_proposition_vocabulary` beyond `lower_evidence_terms` (`5050`) and `terminal_proposition_application_id` (`6543`) (A0229).
- `git show 457adf4a11f:…` at digest commit; this checkout’s line numbers matched the digest citations on read.

SLICE COMPLETE
