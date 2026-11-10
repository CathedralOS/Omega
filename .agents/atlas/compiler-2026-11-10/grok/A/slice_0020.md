# slice_0020

Area digest preamble (read): `.agents/atlas/compiler-2026-11-10/digests/area_09.md` — stage 05 crate path `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/`; short names PCC = `published_facts_crash_calls.rs`, PF = `published_facts.rs`. Group **Published facts, shared, values**. Citations pinned at `457adf4a11f`; excerpts at those lines match current checkout (read; spot-checked with `git show 457adf4a11f` for A0149–A0151 sites).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0149 | published_facts_crash_calls.rs:5205-5207 | REACHABLE | Non–checked-body callee with empty published crash gets `Empty` routes and a call site with zero surviving buckets, not a separate unexamined marker. |
| A0150 | published_facts_crash_calls.rs:815-870 | REACHABLE | Asm intrinsics and atomic-fence calls never become summary invocation sites, so transitive crash summaries omit those edges. |
| A0151 | published_facts_crash_calls.rs:750 | UNDECIDED | Fixed-point loop is capped at `equations.len()` rounds; whether a real dependency graph can fail to converge within that bound was not traced. |
| A0152 | published_facts.rs:6637-6643 | REACHABLE | Index-compat pairs with invalid `semantic_id` or empty argument lists are skipped with no diagnostic for that pair. |
| A0153 | published_facts.rs:412-455 | REACHABLE | Receiver head neither `self` nor a parameter yields `Unknown` provenance, omitting capability verbs that depend on provenance. |
| A0154 | published_facts.rs:7755 | REACHABLE | Missing `MachineBlockingRow` / suspension row / invocation summary defaults operational facts to non-blocking, non-suspending, empty invocations. |
| A0155 | published_facts.rs:8155-8187 | REACHABLE | Crash-route discharge compares `display_name` for `false` leaves, so two distinct expressions with the same display name can drop a route. |
| A0156 | published_facts.rs:8290-8292 | REACHABLE | Operator uses with no `selected_candidate` or no declaration are skipped with no crash-site row or diagnostic from `operator_crashes::build`. |

## A0149

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts_crash_calls.rs:5197-5207` — after published routes are empty, if `supply_mode != CheckedBody`, `target_routes` becomes `SelectedTargetCrashRoutes::Empty` (not private summary).

**Enclosing function (read):** `attach_checked_crash_calls` (`published_facts_crash_calls.rs:5072-5382`), inner per-state loop building `CheckedCrashCallSite` rows (`5136-5342`). `SelectedTargetCrashRoutes::Empty` maps to empty `surviving_summary` (`5307-5308`), then `surviving_buckets` (`5320-5323`) stored on the site (`5326-5340`). Module comment at `5062-5070` (read) says downstream should distinguish unexamined vs proved-crash-free; empty buckets are still attached as checked call records.

**Callers (search):** pattern `attach_checked_crash_calls` in `omega-rust` — 1 definition (`published_facts_crash_calls.rs:5072`), 1 production call (`published_facts.rs:2813` inside crash plan publication); count 2.

**Upward trace (read):** `published_facts.rs:2813-2823` runs after `prepare_crash_summary_graph` (`2786-2794`). `checks.rs:10853-10869` (read) only audits **non-empty** `surviving_buckets` against published ceilings; empty buckets do not participate. Cleanup admission treats missing/unexamined calls separately (`checks.rs:8993-8997`, read). `needs_inferred_summary` (`published_facts_crash_calls.rs:1068-1072`, read) marks checked-body machines with empty published crash for inference, but the `Empty` branch applies when the **callee** is not `CheckedBody` and has no published routes.

**Input that reaches the branch:** A resolved call target that has a local plan and typed machine, empty `target_plan.crash.published()`, and `target_machine.supply_mode` other than `CheckedBody` (e.g. opaque / bodyless provider).

**Smallest program (plain words):** A checked-body machine calls an opaque or externally supplied target that publishes no crash routes and is not checked-body supply; the call is recorded with zero surviving crash buckets, so later ceiling checks never ask that call to justify callee crash causes.

## A0150

**Cited site (read):** `published_facts_crash_calls.rs:815-870` in `machine_invocation_sites` — `continue` for asm intrinsics (`818-823`), atomic-fence calls (`825-826`), byte predicates, build operations, and crash-bearing named operators that already have checked sites (`859-869`).

**Enclosing function (read):** `machine_invocation_sites` (`792-899`) returns `Ok(sites)` or `Err` for unresolved targets / missing declarations; asm and fence paths only skip.

**Callers (search):** pattern `machine_invocation_sites` in `omega-rust` — 1 definition (`792`), 2 production calls (`348` in `unviable_member_dependency`, `413` in `prepare_summary_nodes`); count 3.

**Upward trace (read):** `prepare_summary_nodes` (`380-424`) feeds `prepare_crash_summary_graph` (`240-304`). `infer_private_body_summaries` (`473-696`) closes over `node.invocations` built from those sites. `attach_checked_crash_calls` uses the graph for refinement. No alternate pass (read) re-adds asm/fence calls as crash summary edges in this module.

**Input that reaches the branch:** A flow call row whose typed site is an asm intrinsic builtin, or `atomic_fences::contract` is `Some` for the call.

**Smallest program (plain words):** A machine body whose only outgoing invocations are asm intrinsics or atomic-fence calls contributes no edges to the private crash summary graph, so transitive crash inference for callers ignores those call sites entirely.

## A0151

**Cited site (read):** `published_facts_crash_calls.rs:749-788` — `solve_private_summary_fixed_point` runs `for _ in 0..=equations.len()` dirty propagation, breaks on no change (`784-786`), then returns `resolved` (`788`) even if the loop exhausted without a fixed point.

**Enclosing function (read):** full function `729-789`.

**Callers (search):** pattern `solve_private_summary_fixed_point` in `omega-rust` — 1 definition (`729`), 1 production call (`695` in `infer_private_body_summaries`), 1 test helper call (`1350`); count 3.

**Upward trace (read):** `infer_private_body_summaries` (`473-696`) output feeds `attach_checked_crash_calls` (`5084-5094`). Unit test `dirty_equations_match_original_sweeps_for_exact_dependency_graphs` (`1354+`, read) asserts parity with `original_sweeps` on enumerated graphs only.

**What would decide:** Whether any viable `PrivateSummaryEquation` graph generated from real programs needs more than `equations.len()` relaxation rounds, and whether a partial `resolved` vector can change `surviving_buckets` on downstream calls.

**UNDECIDED because:** I read the cap and test (`1354`) but did not trace equation construction (`506-693`) for cyclic dependencies that might exceed the round bound, nor compare a non-converged return to sound crash summaries.

## A0152

**Cited site (read):** `published_facts.rs:6637-6643` — in `CompatibilityScan::append`, `continue` when `!actual.semantic_id.is_valid()`, `!expected.semantic_id.is_valid()`, or either argument list is empty.

**Enclosing function (read):** `CompatibilityScan::append` (`6588-6760`); instance harvesting `expression_indexed_instances` (`6607-6614`, `5323-5377` region read for empty-instance cases).

**Callers (search):** pattern `build_index_compatibility_facts` in `omega-rust` — 1 definition (`6430`), 1 call (`11198` in `finish_check_facts`); pattern `.append(` inside index compatibility module is only via `build_index_compatibility_facts` scan loops (read).

**Upward trace (read):** `build_index_compatibility_facts` returns `Err(scan.diagnostics)` only when diagnostics non-empty (`6556-6561`); skipped pairs emit nothing. `append_unevidenced_establishment_diagnostics` (`6740-6748`, `6349-6397` read) only fires for `ExpressionNode::Call` with bodyless-domain patterns, not for all skipped `actual`/`expected` pairs.

**Input that reaches the branch:** A value position where one indexed instance has an invalid `semantic_id` or no index arguments while the receiver type still carries a comparable expected instance.

**Smallest program (plain words):** A store or argument position pairs two type-indexed instances in the same family, but one side has no resolved semantic id or empty index arguments; the checker silently skips compatibility for that pair and may accept without an index-compat fact or error.

## A0153

**Cited site (read):** `published_facts.rs:412-455` — `call_receiver_provenance` returns `Unknown` when statement missing (`417-418`), no receiver head (`438-439`), head not `self` (`442-443`), and not any state parameter name (`446-452`).

**Enclosing function (read):** `call_receiver_provenance` (`412-455`); helper `expression_receiver_head` (`461+`, read names only).

**Callers (search):** pattern `call_receiver_provenance` in `omega-rust` — 1 definition (`412`), 1 call (`120` in `build_capability_facts`); count 2.

**Upward trace (read):** `build_capability_facts` (`65-194`) uses provenance at `119-121` to gate `Stores` (`123-140`), `Acquires` (`142-154`), and `Derives` (`156-162`); `Unknown` does not satisfy `MachineOwned` or `CallerProvided` matches. Caller: `finish_check_facts` (`11257`, read). I did not read a stage-05 check that rejects programs solely because capability-flow verbs were under-published (grep `CapabilityFlow` in `checks.rs` — 0 matches, read).

**Input that reaches the branch:** A boundary service call whose receiver path head is a local name or nested field label that is neither the spelling `self` nor a parameter name of the enclosing state.

**Smallest program (plain words):** A boundary call written as `helper.cap.call(...)` where `helper` is a local; provenance is `Unknown`, so capability store/acquire/derive facts are omitted even though authority may still move through that call.

## A0154

**Cited site (read):** `published_facts.rs:7754-7755` — `blocking_row = blocking.iter().find(...)`; `checked_may_block = blocking_row.is_some_and(|row| row.transitive_may_block)` so absent row ⇒ false. Parallel pattern for suspension (`7789-7791`, read) and synchronous invocations (`7825-7837`, `7835-7840` default empty when `invocation_summary` missing, read).

**Enclosing functions (read):** `build_blocking_facts` (`7746-7779`), `build_suspension_facts` (`7781-7815`), `build_synchronous_invocation_facts` (`7817-7880`).

**Callers (search):** each of `build_blocking_facts`, `build_suspension_facts`, `build_synchronous_invocation_facts` — 1 definition each, 1 call each in `finish_check_facts` (`11260-11262`); count 2 per symbol (def + call).

**Upward trace (read):** Rows come from `project_operational_rows(&operational)` (`11258`, `10922-10941`); only machines present in `operational.machines()` get rows. Machines omitted from operational inference still get published facts entries with `checked_may_block == false` / `checked_may_suspend == false` / empty invocation lists (read).

**Input that reaches the branch:** A machine symbol in `program.machines()` with no matching `MachineBlockingRow` after `infer_operational_may_for_dependencies` / operational plan construction.

**Smallest program (plain words):** A machine that may block or suspend per its authored flags or body, but is absent from the operational summary table used to build blocking rows, is published as checked-not-blocking and checked-not-suspending.

## A0155

**Cited site (read):** `published_facts.rs:8155-8187` — `leaf_has_polarity` instantiates a contract label (`8155-8160`) and, for negative polarity, matches facts with `program.expression_table.display_name(expression) == required` (`8184-8187`); positive polarity uses `context_proves_boolean_label` (`8162-8176`).

**Enclosing function (read):** `leaf_has_polarity` (`8130-8190`), called from `expression_has_polarity` (`8095-8127`), used by `statement_entry_route_is_false` (`8026-8055`). Route dropping in refinement: `refine_published_crash_routes` continues (drops guard) when `statement_entry_route_is_false` returns true (`published_facts_crash_calls.rs:2803-2816`, read).

**Callers (search):** `statement_entry_route_is_false` — exported at `8278`; production use in `published_facts_crash_calls.rs:2804` (read); duplicate `leaf_has_polarity` in `checks_operators.rs:1777` (read name only). Search `statement_entry_route_is_false` in `omega-rust`: 2 files (PF module + crash_calls).

**Upward trace (read):** Dropped guards shrink `surviving_buckets` on calls and operator sites, affecting `checks.rs:10853+` coverage only for surviving buckets that remain.

**Input that reaches the branch:** A crash-route guard leaf whose instantiated label string equals another expression's `display_name`, while the proof context carries `BooleanValue { value: false }` for a different expression handle with that same display name.

**Smallest program (plain words):** Two distinct guard sub-expressions that print the same display name; entry-context discharge treats one as provably false and removes the crash route even though the guarded expression is not the one proven false.

## A0156

**Cited site (read):** `published_facts.rs:8289-8292` — `operator_crashes::build` loop: `continue` when `operators.selected_candidate(operator_use)` is `None`. Named-use loop (`8393-8399`, read): `continue` when `declaration_by_symbol` is `None`.

**Enclosing function (read):** `operator_crashes::build` (`8280-8392+`).

**Callers (search):** `operator_crashes::build` — 1 definition (`8280`), 1 production call (`published_facts.rs:2755`); test expectation reuse at `checks.rs:10329` (read). Count 2 production paths.

**Upward trace (read):** `build` returns `Ok(sites)` even when many uses are skipped; only explicit `diagnostics.push` paths reject (`8305+`, read). `checks_operators.rs:1860-1862` (read) also `continue`s when `selected_candidate` is missing for requires-fact checking — parallel omission, not a closing error for crash contracts.

**Input that reaches the branch:** An operator use handle listed in `operators.uses` with a `Crashes` contract on the selected meaning, but `selected_candidate` returns `None`, or a named operator use with no typed declaration.

**Smallest program (plain words):** An expression invokes an operator that carries a `crashes` contract, but operator resolution never produced a selected candidate; fact building skips it and compilation proceeds with no checked operator crash site for that invocation.

## What I did not read

- Full bodies of `validation::infer_synchronous_invocations`, `validation::infer_operational_may_for_dependencies`, and downstream consumers of `CapabilityFlowPlan` outside `published_facts.rs`.
- `context_proves_boolean_label` implementation and wiki/spec language for asm intrinsics, atomic fences, and opaque supply crash obligations.
- `checks.rs` crash-ceiling and cleanup paths beyond the cited `10853-10869` and `8993-9015` excerpts.
- Remaining rows of area_09 digest and other atlas slices.

SLICE COMPLETE
