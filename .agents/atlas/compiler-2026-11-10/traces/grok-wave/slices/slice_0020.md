slice_0020

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0149 | published_facts_crash_calls.rs:5205-5207 | REACHABLE | Non-checked-body callee with empty published crash plan gets `Empty` routes, so the call is still recorded with zero surviving buckets. |
| A0150 | published_facts_crash_calls.rs:815-870 | CLOSED | Asm intrinsics and atomic-fence calls are excluded from the summary graph by design, and separate checks own fence and operator-crash obligations. |
| A0151 | published_facts_crash_calls.rs:750 | UNDECIDED | Iteration is capped at `equations.len()+1` rounds and non-convergence returns the last buckets; convergence for production graphs was not proved. |
| A0152 | published_facts.rs:6637-6643 | REACHABLE | Invalid `semantic_id` or empty argument lists skip the pairwise index-compatibility loop without an error from that loop. |
| A0153 | published_facts.rs:412-455 | REACHABLE | Receiver head that is neither `self` nor a state parameter yields `Unknown`, so some capability flow kinds are not emitted. |
| A0154 | published_facts.rs:7755 | CLOSED | Missing operational rows default checked transitive flags to false, but published blocking/suspension still follow typed `machine.blocks` / `machine.suspends`. |
| A0155 | published_facts.rs:8155-8187 | REACHABLE | False-polarity crash-route discharge matches facts by `display_name` string only, not expression identity. |
| A0156 | published_facts.rs:8290-8292 | CLOSED | Skipped uses are filtered during crash-site materialization; inadmissible or unresolved operators are handled by `resolution_issues` and later checks. |

### A0149

1. Read `attach_checked_crash_calls` in full at `published_facts_crash_calls.rs:5072-5369` (read).
2. Read the `target_routes` selection branch at `published_facts_crash_calls.rs:5171-5307` (read); `SelectedTargetCrashRoutes::Empty` at 5205-5207 maps to `Vec::new()` at 5307.
3. Search `rg '\battach_checked_crash_calls\b' /agent/repos/omega/omega-rust` → 2 matches (definition + `published_facts.rs:2813`).
4. Read caller `build_contract_plans` path at `published_facts.rs:2813-2823` and `finish_check_facts` at `published_facts.rs:11293-11317` (read); did not read `static_callable_crashes::validate_one` in full.
5. Input reaching the branch: resolved call target with local machine and plan, not `target_family_interface`, `target_plan.crash.published()` empty, and `supply_mode != CheckedBody` (read at 5192-5207).

Verdict: **REACHABLE**. A checked-body caller that invokes an opaque or external callee without a published crash ceiling can still get a `CheckedCrashCallSite` whose surviving buckets are empty, so callee crash causes are not refined onto that call. The digest claim is accurate for route materialization; the call row is not omitted (contrast `continue` at 5200-5202 when checked-body lacks a summary).

### A0150

1. Read `machine_invocation_sites` in full at `published_facts_crash_calls.rs:792-899` (read).
2. Read skip sites: asm intrinsic `published_facts_crash_calls.rs:818-823`, atomic fence `825-826`, byte predicate/build ops `828-835`, named crash operators `836-869` (read).
3. Search `rg '\bmachine_invocation_sites\b' /agent/repos/omega/omega-rust` → 3 matches, all in `published_facts_crash_calls.rs` (`348`, `413`, `792`).
4. Read callers `unviable_member_dependency` (`348-353`) and `prepare_summary_nodes` (`413-420`) (read); traced to `prepare_crash_summary_graph` / `infer_private_body_summaries` (read at 473-695, not every helper).
5. Read `checks.rs:12072` `atomic_fences::check` (read); Err path at PCC:859-867 for crash-bearing operators without checked sites (read).

Input: flow-recorded calls that resolve to asm intrinsics, atomic-fence contracts, or already-checked named crash operators.

Verdict: **CLOSED**. These calls are intentionally omitted from private-summary invocation edges; fence checking runs at `checks.rs:12072`, and crash-bearing named operator uses without checked sites fail `machine_invocation_sites` with `Err` (PCC:859-867).

### A0151

1. Read `solve_private_summary_fixed_point` in full at `published_facts_crash_calls.rs:729-788` (read).
2. Read production caller closure at `published_facts_crash_calls.rs:695` inside `infer_private_body_summaries` (read function 473-695).
3. Search `rg '\bsolve_private_summary_fixed_point\b' /agent/repos/omega/omega-rust` → 3 matches (definition, call at 695, test `1350`).
4. Read test `compare` at `published_facts_crash_calls.rs:1348-1352` (read); did not re-run tests.

Input: `equations` slice built from viable summary nodes; when the dirty loop does not stabilize within `equations.len()+1` iterations, the function returns `resolved` from the last pass (read at 750-788).

Verdict: **UNDECIDED**. The cap and “return last state” behavior are clear (read), but I did not establish whether real graphs from `prepare_summary_nodes` can fail to converge within the cap or whether a partial result understates crash causes in a way that lets compilation succeed.

### A0152

1. Read `CompatibilityScan::append` in full at `published_facts.rs:6588-6760` (read).
2. Read skip `continue` at `published_facts.rs:6637-6642` (read); read companion skips in `expression_indexed_instances` at `5354-5377` (read).
3. Search `rg '\bbuild_index_compatibility_facts\b' /agent/repos/omega/omega-rust` → 2 matches (`6430`, `11198`).
4. Read `append_unevidenced_establishment_diagnostics` at `6349-6397` (read): only `ExpressionNode::Call`, bodyless-domain cases.

Input: value positions where both sides of a same-family instance pair have invalid `semantic_id` or empty `arguments`.

Verdict: **REACHABLE**. Those pairs never enter the discharge/diagnostic path at 6644-6708; `append_unevidenced` does not cover the general skip case (read at 6359-6363). `build_index_compatibility_facts` can return `Ok` without an index-compatibility error for that pair.

### A0153

1. Read `call_receiver_provenance` in full at `published_facts.rs:412-455` (read) and `expression_receiver_head` / `expression_path_head` (read 461-479, partial).
2. Search `rg '\bcall_receiver_provenance\b' /agent/repos/omega/omega-rust` → 2 matches (`120`, `412`).
3. Read caller `build_capability_facts` loop at `published_facts.rs:89-170` (read).

Input: service-reach boundary call whose receiver path head is a local or other identifier that is not spelled `self` and does not match any state parameter name on the enclosing state (read 442-454).

Verdict: **REACHABLE**. `Unknown` provenance prevents `MachineOwned` / `CallerProvided` paths for `Stores` and some `Acquires`/`Derives` combinations (read 123-170), under-reporting capability flows while `build_check_facts` still succeeds.

### A0154

1. Read `build_blocking_facts`, `build_suspension_facts`, and `build_synchronous_invocation_facts` at `published_facts.rs:7746-7879` (read in full for those three functions).
2. Read `project_operational_rows` at `published_facts.rs:10922-10941` (read).
3. Search `rg '\bbuild_blocking_facts\b' /agent/repos/omega/omega-rust` → 3 matches (definition, import, `11262`).

Input: `program.machines()` entry with no matching `MachineBlockingRow` / `MachineSuspensionRow` from `project_operational_rows`.

Verdict: **CLOSED**. Missing rows make `checked_may_block` / `checked_may_suspend` false (read 7754-7755, 7789-7791), but for non-interface machines `published_may_block` / `published_may_suspend` still come from typed `machine.blocks` / `machine.suspends` (read 7756-7760, 7792-7796). Synchronous invocation targets use `infer_synchronous_invocations` (7820-7840), not operational row lookup. The digest’s blanket “not blocking” oversimplifies the published side.

### A0155

1. Read `leaf_has_polarity` at `published_facts.rs:8130-8189` and callers `expression_has_polarity` / `statement_entry_route_is_false` (read 8026-8127).
2. Read `context_proves_boolean_label` and `true_expression_proves_label` at `published.rs:52-138` (read).
3. Read `retained_operator_crash_routes` guard loop at `published_facts.rs:8558-8614` (read, partial).

Input: false-polarity leaf whose `required` label from `instantiate_operator_contract_expression_label` equals `program.expression_table.display_name(expression)` for a different `expression` handle (read 8185-8187).

Verdict: **REACHABLE**. A surviving crash route guard can be dropped on display-name collision even when the fact’s expression is not the guard producer, weakening crash refinement while check-fact construction continues.

### A0156

1. Read `operator_crashes::build` at `published_facts.rs:8280-8440` (read through both loops; not every helper below 8522).
2. Search `rg '\boperator_crashes::build\b' /agent/repos/omega/omega-rust` → 2 matches (`2755`, tests in `checks.rs`).
3. Read `check_operator_resolution` at `checks_operators.rs:2835-2879` (read); traced call from `checks.rs:12162` (read).

Input: operator use with `operators.selected_candidate(operator_use) == None` (8290-8291), or named use whose `declaration_by_symbol` fails (8394-8398).

Verdict: **CLOSED**. The build loop only materializes crash sites for resolved crash-contract uses; skipped rows are not program acceptance. `check_operator_resolution` appends diagnostics from `resolution_issues()` for ambiguous/inadmissible meanings (`2839-2850`), and `machine_invocation_sites` rejects crash-bearing named operators without checked sites (PCC:859-867). I did not enumerate every `CheckedOperatorResolutionStatus::Missing` path for crash operators.

### What I did not read

- Full bodies of `refine_published_crash_routes`, `static_callable_crashes::validate_one`, and `infer_synchronous_invocations`.
- Callers above stage-05 `checks.rs` after `CheckFacts` construction (permission walk, full content checks).
- Whether operational inference always emits a `MachineOperational` row for every typed machine symbol.
- Remaining callees of `prepare_crash_summary_graph` beyond the `machine_invocation_sites` chain.

SLICE COMPLETE
