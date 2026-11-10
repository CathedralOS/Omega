# slice_0035

Area digest preamble (read): Psi stages 06–08 lower checked trees to Terminal Psi; bare names live under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`, with `07/` and `08/` subpaths as in `digests/area_10.md`. Digest citations target commit `457adf4a11f`; opened lines in the current checkout match the cited excerpts (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0264 | unit_arguments.rs:4293-4306 | REACHABLE | Empty lowered proof actuals on a jump can be filled from the caller’s erased proof roster without re-checking witnesses. |
| A0265 | unit_arguments.rs:1849-1855 | REACHABLE | `ScalarArgumentCaptures::values` omits unset ordinals, so a composed call can lower with a shortened scalar operand list. |
| A0266 | unit_composed_control.rs:365 | CLOSED | Skipping `require_closed` when `all_paths_crash` matches the stage-05 borrowed-window rule at the same predicate. |
| A0267 | unit_operation_frame.rs:2889 | REACHABLE | `SuspensionCrossingValueOrigin::CallArgument` live values are skipped and never enter the suspension source frontier. |
| A0268 | 07/psi_optimization.rs:48-62 | CLOSED | No-op optimization variants do not gate compilation acceptance; stage-08 sealing still validates the semantic module. |

## A0264

**Cited site (read):** `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit_arguments.rs:4293-4306` — private `completion_roster`: if `actuals` is empty and `roster` is non-empty, it builds `ProofTerm::Formal { position: parameter.proof_position }` for each `CheckedErasedProofParameterPlan` in `roster`; otherwise returns `actuals` unchanged.

**Enclosing context (read):** `completion_roster` sits in the scalar branch-state emission module; jump/qualify terminators assign `*proof_actuals` or `erased_proof_arguments` after `lowered_proof_terms(...)?` (e.g. `3754-3757`, `3802-3809`, `3969-3975`).

**Callers (search):** pattern `\bcompletion_roster\b` in `omega-rust` — 1 definition (`unit_arguments.rs:4293`), 4 call sites (all `unit_arguments.rs`: `3754`, `3802`, `3969`, `3973`); 0 other matches (inferred: no external callers).

**Upward trace (read):** `emit_state` (`unit_arguments.rs:3394-3416+`) lowers each `LoweredScalarBranchTerminator::Jump` / `Qualify` / related arms, runs `lowered_proof_terms` on the branch’s proof-term list (`scalar_lowering.rs:2175-2184`, read), then passes the resulting `Vec` through `completion_roster` with `source_erased_proof_formals` (caller edge roster). There is no further proof-obligation check on the synthesized `Formal` terms in this path (read). Lowering failure surfaces as `LoweringError`; success continues block/terminator installation into the lowered machine.

**Input that reaches the branch:** A scalar branch terminator whose lowered proof-term list is empty while `source_erased_proof_formals` is non-empty (e.g. jump with no explicit proof witnesses on the branch state, but the successor edge still carries an erased-proof roster).

**Smallest program (plain words):** A machine whose scalar control fork should supply distinct proof witnesses at a jump, but the branch lowering emits no proof terms; stage 06 substitutes the caller’s formal erased-proof positions and the program still lowers to Terminal Psi.

## A0265

**Cited site (read):** `unit_arguments.rs:1849-1854` — `ScalarArgumentCaptures::values` iterates `self.slots` with `filter_map(|slot| slot.and_then(|slot| values.get(slot)).copied())`, so ordinals with `None` slots are dropped entirely (not placeholder gaps).

**Enclosing impl (read):** `ScalarArgumentCaptures` (`1837-1854`); `record` (`1842-1846`) sets `slots[ordinal] = Some(slot)` when an argument value is captured.

**Callers of `values` (search):** pattern `ScalarArgumentCaptures` in stage-06 — definition `1837`; `values(` used at `unit_composed_control.rs:792` (read). `record(` at `unit_composed_control.rs:664` during `Step::Argument`.

**Upward trace (read):** Composed operation schedule (`unit_composed_control.rs:595-819`): each `Step::Argument { operation, ordinal }` evaluates an operand and `record(ordinal, values.len())`; at `Step::Call` / `Step::Operation`, `staged_scalar_slots[operation_index].values(values)` becomes `staged_scalars` passed into `literal_arguments::evaluate` / the frame call path (`792-818`). If some ordinals never received `record`, `values()` returns a shorter vector in capture order, not a fixed-width tuple aligned to operand indices.

**Bundled digest note (read, not primary cite):** same row lists `744-753` (`filter_map` dropping partition-only cleanup rows), `1532-1535`, and `5191-5207` (invalid symbol adoption in `record_owned_home`); stage-05 guard for `CheckedPartition` at `unit.rs:3703-3716` was not confirmed against all producers (digest item 90).

**Input:** Composed call emission where at least one planned scalar operand ordinal has no `record` entry in `staged_scalar_slots[operation]` but the call step still runs.

**Smallest program (plain words):** A composed machine call that should pass three scalar operands, but only two capture slots were recorded; lowering supplies two scalars as the staged operand list and still emits the call.

## A0266

**Cited site (read):** `unit_composed_control.rs:356-368` — `OperationSequence::finish_state` matches `state.terminator.all_paths_crash(&state.operations, &checked.facts.flow.guarded_exits)`: `Some(true)` ⇒ `Ok(())` without `require_closed()`; `Some(false)` ⇒ `self.require_closed()` (`352-354`, `windows.require_closed()`).

**`all_paths_crash` (read):** `omega-rust/psi/compilation/representations/checked-trees/src/flow_terminal_units.rs:430-445` — returns `Some(false)` when external completion exists; otherwise delegates to `abandons_custody`.

**Callers of `finish_state` (search):** pattern `finish_state` in `unit_composed_control.rs` — definition `356`, calls at `873`, `3953`, `8223` (read).

**Stage-05 closing check (read):** `execution_terminal_unit_state_graph.rs:9883-9886` — if `!sequence.windows.is_closed()` **and** `!terminator.all_paths_crash(...)?`, state-graph checking returns `None` (reject). If `all_paths_crash` is true, unclosed borrowed windows are allowed through checking.

**Related site in digest (read):** `unit_composed_control.rs:3231-3236` skips `require_closed` when completion is not `Terminator::Crash` only for that arm path; crash completions skip window closure at emission time as well.

**Input:** Composed state whose terminator’s `all_paths_crash` is `Some(true)` while `BorrowedWindowLedger` is not closed.

**Why CLOSED:** Any checked program that reaches lowering with unclosed windows and `all_paths_crash == true` has already passed the dual of the same predicate at stage 05 (`9883-9886`); lowering’s `365` branch does not introduce a new acceptance path relative to checking.

## A0267

**Cited site (read):** `unit_operation_frame.rs:2889-2891` — in `retain` (`2809-3033`), loop over `crossing.live_values`; for `SuspensionCrossingValueOrigin::CallArgument { .. }` the arm is `continue` before any home/`authored_position` binding work.

**Enclosing function (read):** `retain` builds `LoweredSourceFrontierBindings` for a suspension call coordinate; called from `suspension_source_frontier` on `OperationFrame` (`2749-2767`) and `OperationEnvironment` (`2771-2806`).

**Callers (search):** pattern `suspension_source_frontier` in `omega-rust` — 4 references in `unit_operation_frame.rs` (`2749`, `2771`, `1431`, `1543` read); 3 in `unit_arguments.rs` (not opened).

**Consumers (read):** `source_call_record` / `record_source_call` (`1413-1449`) attach the frontier to `operations.record_source_call`; module no-normal and structural call paths use it (`1528-1556`).

**Checker side (read):** `checks.rs:6280-6305` — `CallArgument` participates in `value_frontier` loan walking; `checks.rs:7206` — `CallArgument` has `available_before == false` for carry-value naming, but `crossing.live_values` can still list `CallArgument` origins (read `2856-2891` loop input).

**Bundled digest note (read):** same row also cites `2971-2979`, `2996-2998`, `3011-3017` (other `continue` paths when home/authored position missing), `3178-3228` (call-ensured membership skips), `8085-8091` (cleanup skip); consumers of incomplete frontiers were not fully traced (digest item 92).

**Input:** May-suspend call whose checked `suspension_crossings` row includes a `live_values` entry with `CallArgument` origin that should appear in the lowered source frontier for custody/source-call metadata.

**Smallest program (plain words):** A suspending call where the checker recorded a call-operand live value in the crossing set; stage 06 omits it from the frontier entirely, yet still records/emits the call.

## A0268

**Cited site (read):** `omega-rust/psi/compilation/pipeline/07_lowered-psi-to-lowered-psi/src/psi_optimization.rs:48-51` — `ProofCheckElision | StateSpecialization | RepresentationSpecialization | LoopInvariantScalarMotion` are no-ops (`{}`); execution record still built at `60-62` via `PsiOptimizationExecutionRecord::new(selections, ...)`.

**Enclosing function (read):** optimization driver `psi_optimization.rs:32-63` (read `30-64`).

**Compilation acceptance path (read):** `08_lowered-psi-to-terminal-psi/src/lib.rs:104-117` — `finalize_terminal_program` validates `activation_storage` on the semantic module and seals the artifact; it does not require that each selected optimization pass mutated the IR.

**Bundled digest note (read):** item 93 also cites `08/result_policy_verification.rs:334-336` — non-`Expression` `source_site` on a float boundary call hits `continue` and skips adapter verification; `verify_boundary_result_policies` has no in-repo production callers (digest/worklist U0033, grep: 4 hits in 2 files in stage-08 crate only). Whether such a `source_site` can occur in lowered output was not traced (digest: “not traced”).

**Why CLOSED (primary cite):** Selecting no-op optimization passes still lowers/compiles the same module; acceptance is gated by stage-08 semantic validation (`104-117`), not by whether those four passes changed the carrier.

## What I did not read

- Stage-05 rules that would forbid empty branch proof terms before stage 06 (would decide strength of A0264 vs checker-only CLOSED).
- Full composed `argument_schedule` invariants proving whether a `Call` step can run with missing `Step::Argument` ordinals (would decide A0265 reachability from schedules alone).
- Downstream consumers of `LoweredSourceFrontierBindings` beyond `record_source_call` / invocation completion (would decide soundness impact for A0267).
- Corpus/xtask or test harness calls to `verify_boundary_result_policies`, and lowering paths that emit float boundary calls with non-`Expression` `source_site` (bundled A0268 float clause).
- Remaining bundled sites in digest items 91 and 92 outside each row’s primary `cited` line.

SLICE COMPLETE

