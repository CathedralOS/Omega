# slice_0021

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0157 | published_facts.rs:9551-9568 | REACHABLE | Ensures/domain rows publish with default empty evidence when the domain lookup fails or the payload is not domain membership. |
| A0158 | published_facts_crash_entry_values.rs:1014 | REACHABLE | `statement_may_overwrite` treats every `Assembly` statement as non-writing, so a binding can look stable across an assembly write. |
| A0159 | published_facts_crash_entry_values.rs:2211-2220 | REACHABLE | Top-level float bindings skip the mutable-storage write scan after reference peel, so a mutating float update can be ignored. |
| A0160 | published_facts_crash_entry_values.rs:1296-1299;1246-1251 | REACHABLE | Call write analysis zips parameters to arguments by index only and walks expression subtrees through `VALUES` edges only. |
| A0161 | shared.rs:1028-1030 | REACHABLE | Product-operand late bindings exit finalization with `None` and the post-pass unresolved check does not enumerate those occurrences. |
| A0162 | shared.rs:2181-2198;2253-2292;2648 | UNDECIDED | Stage 05 forces builtin operator resolution on this ladder; whether user overloads are legal in the language was not read here. |
| A0163 | shared.rs:489-491;2912-2923;4685-4687 | REACHABLE | Without a contextual root, name-path finalization returns the direct resolver symbol; assembly-fact and boundary-candidate paths are separate. |
| A0164 | values.rs:1016-1018 | REACHABLE | `StructuralIdentity` is true on syntactic tree equality (`left == right`), not on whether storage was written between reads. |

## A0157

**Cited site (read):** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/published_facts.rs:9542-9594`, function `qualification_evidence::call_contract_evidence`.

**Enclosing function (read):** For `is_ensures == false`, lines `9551-9552` return `Some(QualificationEvidence::default())` for every non-ensures contract row. For ensures rows, non-`ContractDomainMembership` payloads at `9563-9564` and missing `domain_definition` at `9566-9567` also return default evidence. Only when a domain exists does `call_qualification_source` run (`9570-9593`).

**Callers (search `omega-rust`, `\bcall_contract_evidence\b`, read):** 4 matches — 1 definition (`published_facts.rs:9542`), 1 comment reference (`published_facts.rs:5391`), 2 production calls in `published.rs:8068` (proof output ensures, `is_ensures: true`) and `published.rs:8588` (`append_call_semantic_contract_refs`, `is_ensures` from `FactOrigin::CallEnsures`). Both callers append facts when `evidence` is `Some`; default evidence is `Some`, not filtered by `is_independent_receipt` at append (`published.rs:8597-8605`, `evidence.rs:227` read for default not being an independent receipt).

**Upward trace (read):** `checking.rs:1067` calls `build_check_facts`, which drives semantic fact publication in `published.rs`. A call-site ensures fact whose domain symbol has no `domain_definition` still gets a published fact with empty qualification evidence instead of being dropped.

**Input that reaches the branch (plain words):** A machine call with an ensures clause naming domain membership for a domain symbol the program tables do not define, or a requires/non-domain contract row processed through the same helper with `is_ensures` false.

**Verdict:** REACHABLE — such a program can complete checking with contract facts carrying default evidence that should require real qualification.

## A0158

**Cited site (read):** `published_facts_crash_entry_values.rs:1006-1078`, function `statement_may_overwrite` (via `statement_may_overwrite_place` at `966-980`).

**Enclosing function (read):** `StatementNode::Assembly(_) => false` at `1013-1014` before assignment, call, and expression arms that delegate to `place_interferes` / `expression_may_overwrite`.

**Callers (search `omega-rust`, `\bstatement_may_overwrite_place\b`, read):** 8 matches — definition/re-export in `published_facts_crash_entry_values.rs` (`966`, `1316`, `141`), plus uses in `published_facts_crash_calls.rs` (`1759`, `2112`, `3570`, `3837`, `3986`). Production uses include crash-entry scalar evaluation (`crash_entry_values.rs:141-147`) and call-site operand stability (`crash_calls.rs:3837`).

**Upward trace (read):** `storage_holds_bound_value` (`989-1004`) calls `statement_may_overwrite` over statement windows; `local_read_initializer` (`2010-2037`) uses `storage_holds_bound_value` after `observation_requires_write_scan`. False on assembly makes the window appear clean.

**Input that reaches the branch (plain words):** A state where a binding must stay at its entry value for crash-entry provenance, with an `Assembly` statement between the binding and a read that (in source semantics) mutates the same local; the scan never marks the assembly as overwriting.

**Verdict:** REACHABLE.

## A0159

**Cited site (read):** `published_facts_crash_entry_values.rs:2206-2246`, function `observation_requires_write_scan`.

**Enclosing function (read):** When `field_path` is empty, peeled primitive `accepts_float_literal()` at `2211-2220` returns `Some(false)` (no write scan). Otherwise mutable-reference stable observation or `projection_requires_write_scan` applies.

**Callers (search `omega-rust`, `\bobservation_requires_write_scan\b`, read):** 3 matches — definition (`2206`) and calls in `local_read_initializer` (`2023`) and parameter-read path (`2282`, same file, read).

**Upward trace (read):** `call_arguments_may_overwrite` (`1280-1313`, read) pairs parameters with arguments positionally and peels references in a loop (`1301-1310`) without name/selector alignment — supports A0160 separately. For A0159, float locals/parameters bypass the scan that gates `storage_holds_bound_value`.

**Input that reaches the branch (plain words):** A mutable or borrow-tracked `f64` local (or empty-path observation of a float binding) where an intervening statement writes through `&mut f64`; write scan is skipped and provenance may still treat the binding as stable.

**Verdict:** REACHABLE.

## A0160

**Cited sites (read):** `published_facts_crash_entry_values.rs:1296-1313` (`call_arguments_may_overwrite`) and `1246-1251` (default arm in `expression_may_overwrite` using `SubtreeEdges::VALUES` only).

**Enclosing functions (read):** `call_arguments_may_overwrite` zips `parameters.iter().zip(arguments)` with no selector or name alignment (`1296-1299`). `expression_may_overwrite` walks pending expressions; non-handled node kinds push only `VALUES` children (`1246-1251`).

**Callers (read):** `statement_may_overwrite` call arm (`1053-1077`) calls `call_arguments_may_overwrite`; `expression_may_overwrite` is used throughout the same module (`1016-1044`, `1053-1077`, read).

**Upward trace (read):** Same provenance chain as A0158/A0159 through `storage_holds_bound_value` and crash-entry evaluation.

**Input that reaches the branch (plain words):** A call whose formal parameters and actual arguments are not in the same semantic order but share arity, or whose side-effecting subexpressions live outside `VALUES` child edges, so the write scan pairs the wrong argument or misses a write.

**Verdict:** REACHABLE.

## A0161

**Cited site (read):** `shared.rs:1028-1030` inside `finalize_checked_authored_selections` (`973-1426`).

**Enclosing function (read):** After loading `product_operands` from `validation::execution_projection::product_request_operand_occurrences` (`994-995`), occurrences in that set return `Ok((found, None))` without binding a checked target (`1028-1030`).

**Callers (search `omega-rust`, `\bfinalize_checked_authored_selections\b`, read):** 3 matches — definition (`shared.rs:973`), invocation `checking.rs:1215`, and one other reference in `shared.rs:973` module path (read).

**Closing check (read):** Post-loop unresolved diagnostic at `1385-1422` scans `selections.iter()` for any remaining `SelectionTarget::LateBound(_)` not in `descriptor_occurrences` and not toolchain-derived wire codec. Product-operand occurrences are never finalized and are not named in that predicate.

**Input that reaches the branch (plain words):** A build/product operand expression whose authored declaration selection stays `LateBound` and whose occurrence is listed in `product_request_operand_occurrences`; checking can finish without resolving that selection.

**Verdict:** REACHABLE.

## A0162

**Cited sites (read):** `shared.rs:2174-2292` (`checked_operator_target`), `2181-2198` (seven binary operators → `BuiltinOperator`), `2253-2292` (primitive/contextual builtin fallbacks), `2648` (`ExpressionNode::Unary(_) => return true` in `operator_has_no_authored_spelling_candidate`).

**Enclosing function (read):** Operator finalization ladder returns `SelectionIntrinsic::BuiltinOperator` for listed bitwise/logical/shift binaries before use-fact resolution, then falls through to primitive operand heuristics and “no authored spelling” (`2253-2291`, `2648-2657`).

**Callers (search `omega-rust`, `\bchecked_operator_target\b`, read):** 4 matches in `shared.rs` (definition `2174` and resolution paths `1132` area / operator binding arms, read).

**Upward trace (read):** Invoked from `finalize_checked_authored_selections` when `LateBinding::CheckedOperator` (`1031-1039`, read). Digest note at area_09 digest line `202` (read): whether the language allows user overloads for those operators was not checked.

**What would decide it (not read):** Language specification or validation rules for user-declared operators on the listed primitives/unary forms.

**Verdict:** UNDECIDED on unsound acceptance; the builtin-forcing branch is read and reachable in code if overloads are legal.

## A0163

**Cited sites (read):**

1. `shared.rs:489-491` — `checked_name_path_segment_target` (`446-515`): when `contextual_root` stays `None`, returns `direct` from `validation::resolve_name_path_member_symbol` (`452-454`, `489-491`).

2. `shared.rs:2912-2923` — `collect_checked_proof_view_call_selections` (`2885+`): iterates machines/states and extends `fact_roots` from `StatementNode::AssemblyFact` proof roots (`2917-2920`).

3. `shared.rs:4685-4687` — `candidates_for_application` (`4646+`): `continue` when no `boundary_declarations` row matches (`4685-4686`).

**Callers (search, read):** `\bchecked_name_path_segment_target\b` — 3 matches (`shared.rs` definition and `1135` finalization). `collect_checked_proof_view_call_selections` called from the same finalization module (`939`, read). `candidates_for_application` is boundary-operator wiring (read call chain from `shared.rs` boundary module, not fully traced to `checking.rs`).

**Upward trace (read):** (1) Static path segment late binding uses the returned symbol at `1135-1140`. (2) Assembly-fact roots enlarge proof-view collection before view-call minting (`2880-2883` comment, read). (3) Missing declaration drops a body candidate only (`continue`), not the whole compile.

**Input that reaches the branch (plain words):** (1) A name-path segment that needs statement-local contextual root disambiguation but only the global direct symbol is valid. (2) Proof-view calls rooted in assembly facts. (3) Boundary operator application with no matching checked declaration row.

**Verdict:** REACHABLE for (1); (2) and (3) are conservative collection/candidate filtering without a single acceptance bug traced here.

## A0164

**Cited site (read):** `values.rs:1016-1018` in `evaluation::boolean` (`995-1027`).

**Enclosing function (read):** `CheckedBooleanExpression::StructuralIdentity { left, right }` evaluates to `Some(true)` only when `left == right` (tree equality on `CheckedBooleanExpression`, `read`).

**Construction path (read):** Mathematical identity calls lower to `CrashPredicateExpression::structural_identity` in `published_facts_crash_calls.rs:4960-4965` (two lowered subject operands). Checked forms are substituted/ evaluated via `evaluate_checked_scalar` in crash and predicate paths (`published_facts_crash_entry_values.rs:134`, `published_facts_boolean_predicates.rs:503`, read).

**Callers (read):** `evaluate_checked_scalar` / `evaluate_checked_boolean` re-export at `values.rs:3450-3451`; many production uses in stage 05 (crash, flow, predicates; grep `evaluate_checked_scalar` in `05_typed-trees-to-checked-trees`, read count ≥ 15 files).

**Input that reaches the branch (plain words):** Two crash or proof operands that are syntactically the same checked tree (e.g. two `StorageRead` of the same symbol) while mutable storage changed between the corresponding runtime reads; identity evaluates true without consulting write facts.

**Verdict:** REACHABLE.

## What I did not read

- `git show 457adf4a11f:...` line drift check (current checkout lines matched cited ranges on read).
- Omega language-guide chapters on assembly side effects, product build operands, and user operator overloading.
- Full `validation::call_qualification_source` and downstream proof-admission consumers outside stage 05 cited call sites.
- Entire `finalize_checked_authored_selections` parallel resolution arms beyond product-operand and operator branches.
- Pre-stage-05 validation that might reject programs before `call_contract_evidence` or finalization runs.

SLICE COMPLETE
