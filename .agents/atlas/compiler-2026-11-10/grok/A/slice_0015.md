# slice_0015

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0109 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:1273-1353 | REACHABLE | Missing operational rows get `Default` suspension (both false) and the same states skip acknowledgement validation before summaries attach. |
| A0110 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:1469,1506,1524-1539 | REACHABLE | Non-static store destinations leave the prior snapshot; an empty predecessor list still inserts a `Join` node. |
| A0111 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:1647-1678,1730-1791 | REACHABLE | Unresolved storage yields `Some(empty)` so alias expansion at 1780 never runs; the prover may see only the unexpanded place. |
| A0112 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:2741-2775 | REACHABLE | Exit rebasing with `assumptions: false` still `append_fact`s machine-rooted places when no origin target resolves. |
| A0113 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:3729-3734 | REACHABLE | Unresolvable selection makes `possible` false; contract and multiplicity checks `continue` and omit the conditional obligation. |
| A0114 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:4146-4174,4292-4296 | REACHABLE | Expression-form operator effects skip invalidation on missing operand rebuild or missing `&mut` place; statement form treats that as unknown writes. |
| A0115 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:4432-4449,7061-7065 | REACHABLE | Operand-less `ensures` expressions publish at `FactPlace::Unknown`, which write invalidation never retires. |
| A0116 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs:4652-4656 | UNDECIDED | Early `return` drops the whole state flow row when borrow has no state; whether a checked machine state can lack that row was not traced. |

## A0109

**Digest (read):** Area 9 preamble (`digests/area_09.md`) maps `F` to `flow.rs` in stage 05; claim at digest line 147 matches slice text.

**Cited site (read):** `reach::attach_reach_summaries` at `flow.rs:1259-1356` (`flow.rs:1273-1353` in digest). Cites match this checkout.

When `operational_state` lookup by `state.state_symbol` fails (`flow.rs:1275-1279`), `state.suspension` and `state.blocking` use `unwrap_or_default()` (`flow.rs:1293-1307`). Per-call rows do the same when `operational_call` is missing (`flow.rs:1339-1350`). `ServiceReachSummary` uses the same pattern (`flow.rs:1268-1273`, `1321-1326`).

**Default (read):** `SuspensionSummary` and `BlockingSummary` derive `Default` with both boolean fields false (`omega-rust/psi/foundation/language-semantics/src/lib.rs:2053-2062`).

**Callers (read):** `rg 'attach_reach_summaries' omega-rust` → 5 matches: definition and re-export in `flow.rs`, production calls at `flow.rs:349` (`builder::build_flow_facts`) and `published_facts.rs:11231` (`finish_check_facts` path).

**Upstream accept path (read):** In `published_facts.rs:11228-11236`, `validate_call_acknowledgements` runs immediately before `attach_reach_summaries`. In `checking::validate_call_acknowledgements` (`checking.rs:39-77`), a state with no operational summary hits `else { continue; }` at `checking.rs:52-53` and never validates its calls.

**Downstream (read):** `checks.rs:7470-7474` skips suspension-crossing borrow work when both `direct_may_suspend` and `transitive_may_suspend` are false on the call flow row.

**Input (read):** A machine state that is absent from `OperationalPlan.states` but whose body can suspend (or host suspending calls) is published with default non-suspending summaries and never saw acknowledgement validation for that state.

**Verdict:** REACHABLE — checking can treat the state and its calls as non-suspending while operational validation was skipped for that state.

## A0110

**Cited site (read):** `stored_values::append_store` (`flow.rs:1457-1488`) and `append_preserved_binding` (`flow.rs:1495-1521`) return `previous` unchanged when any destination segment fails `place_segment_has_static_identity` (`flow.rs:1464-1469`, `flow.rs:1501-1506`). `stored_values::join` (`flow.rs:1524-1539`) only short-circuits when all predecessors are the same handle; an empty slice falls through to `FlowStorageOperation::Join { predecessors }` with an empty predecessor span (`flow.rs:1533-1538`).

**Callers — `append_store` (read):** `rg 'append_store\(' …/05_typed-trees-to-checked-trees` → 4 matches: definition `flow.rs:1457`, statement path `flow.rs:5637`, expression paths `flow_expression.rs:4981`, `:4989`.

**Callers — `join` (read):** `rg 'stored_values::join' omega-rust` → 3 matches: import and `CallStoredValue::Alternatives` arm `flow_expression.rs:5002`, `join_storage_prefixes` `flow_expression.rs:10393`; definition `flow.rs:1524`.

**Input (read):** A store to a destination whose path includes a non-static segment produces no new snapshot row, so downstream reads still see the older snapshot. An alternative branch with zero stored values (or `join_storage_prefixes` with an empty prefix list) calls `join` with `predecessors == &[]`, recording an empty join node rather than failing flow construction.

**Verdict:** REACHABLE — storage timelines can silently omit writes or contain empty joins that later interpretation treats like ordinary merge points.

## A0111

**Cited site (read):** `stored_views::reference_places_at_snapshot` (`flow.rs:1634-1687`) returns `Some(Vec::new())` on `StoredOperand::Unavailable` (`flow.rs:1647-1647`), failed admission (`flow.rs:1649-1650`), invalid capture (`flow.rs:1664-1665`), unresolved formation (`flow.rs:1674-1675`), and empty selection (`flow.rs:1677-1678`). `captured_before_snapshot` propagates empty as `Some(Vec::new())` (`flow.rs:1618-1621`, `1620-1621`). `before_snapshot` wraps that as `Some(...)` (`flow.rs:1582-1594`).

**Cited site — `address_before_statement` (read):** `flow.rs:1730-1791`. For each pending place, `before_snapshot` (`flow.rs:1773-1776`); when it returns `Some(places)` with `places.is_empty()`, the expansion loop at `flow.rs:1780-1784` runs zero times, `replaced` stays false, and the original `source` is pushed at `flow.rs:1787-1788` (digest “vanishes” read as: no expanded alias addresses are produced at `1780-1784`, not that the whole function returns empty).

**Callers (read):** `rg 'address_before_statement' omega-rust` → 2 matches: definition `flow.rs:1730`, production use `checks_contracts_prover.rs:6598` inside `receiver_places` (propagates `None` with `?`).

**Callers — `reference_places_at_snapshot` (read):** `rg 'reference_places_at_snapshot' omega-rust/psi` → 6 matches: definition and `captured_before_snapshot` in `flow.rs`, four sites in `flow_expression.rs`.

**Input (read):** Contract proving that needs receiver places before a statement; storage resolution hits `Unavailable` or empty capture inventory so `reference_places_at_snapshot` returns `Some(empty)`; `address_before_statement` never replaces through live-view bindings and only retains the unexpanded canonical place.

**Verdict:** REACHABLE — the prover can proceed with `Some(vec![unexpanded])` where a fully expanded address set was required for sound invalidation or membership reasoning.

## A0112

**Cited site (read):** `entry_origins::rebase_contexts` (`flow.rs:2604-2796`). For each machine-scoped fact copied into an exit/state point, parameter roots are collected (`flow.rs:2710-2737`). Rebasing `place.root` happens only when `origins.iter().find` matches (`flow.rs:2726-2729`) or `invocation_referent` applies under `!assumptions` (`flow.rs:2730-2735`). Otherwise the fact keeps its prior root. At `flow.rs:2740-2773`, each `required_roots` entry still runs; `target` is `unwrap_or_default()` (`flow.rs:2744-2747`); `complete &= target.is_valid() && !assumptions` (`flow.rs:2759`). Facts are always `semantic.append_fact`’d (`flow.rs:2772-2773`). With `assumptions == false`, `if complete || !assumptions` always appends the rebased context (`flow.rs:2775-2781`).

**Callers (read):** `rg 'rebase_contexts\(' omega-rust/psi` → 3 matches: definition `flow.rs:2604`, state entry `build_state_flow_fact` `flow.rs:4688` with `assumptions: true`, exit ensures `flow_exits.rs:1502` with `assumptions: false`.

**Input (read):** An exit-context rebase (`flow_exits.rs:1502-1509`) for a contract fact whose place root is a state parameter symbol with no row in `origins` and no `invocation_referent` fallback; the fact is published at the exit point still rooted at the machine-level symbol.

**Verdict:** REACHABLE — exit-visible contract facts can remain tied to unrebased parameter roots while still entering the active semantic context list.

## A0113

**Cited site (read):** `SelectionConditions::possible` (`flow.rs:3711-3749`). After cache lookup, `let Some(selection) = self.first_selection(condition) else { return false; }` (`flow.rs:3729-3730`). `first_selection` returns `None` for `Condition::True` / `Condition::False` and defers to arms for `All`/`Any` (`flow.rs:3800-3816`). Invalid conditions and missing `operand_alternatives` spans also yield false (`flow.rs:3719-3720`, `3732-3733`).

**Callers (read):** `rg 'conditions\.possible\(' omega-rust/psi` → 6 matches: `checks_multiplicity.rs:6568`, `:6592`; `checks_contracts.rs:5760`, `:6013`, `:6091`, `:6146`. Digest cites `6568`, `6592`, `5760`; those read as `if conditions.possible(...) { ... } else { continue; }` or guarded emission (`checks_contracts.rs:5760-5761`).

**Input (read):** A structural-case overlap or membership condition whose selection node cannot be resolved to an operand-alternative span (or reduces to true/false sentinels without arms); `possible` is false, so the checker omits the conditional predicate branch rather than treating overlap as unknown-required.

**Verdict:** REACHABLE — obligations gated on `possible` true are not emitted when selection structure is unresolvable, so checking may proceed without those predicates.

## A0114

**Cited site (read):** `operator_calls::apply_named_operator_call_effects` (`flow.rs:4133-4221`). Missing operator (`flow.rs:4146-4154`) or `named_operator_call_operands` failure (`flow.rs:4156-4164`) returns early without invalidation. `mutated_places` uses `.filter_map(...).flatten()` on optional places (`flow.rs:4166-4174`), omitting mutable operands with `place: None`. Contrast `operator_statement_call_mutated_places` (`flow.rs:4273-4296`): same operand list, but `map(|operand| operand.place).collect()` on mutable operands returns `None` if any mutable place is missing (`flow.rs:4292-4296`); comment at `flow.rs:4229-4233` states `None` means unknown overlap with all prior writes.

**Operand rebuild (read):** `named_operator_call_operands` (`flow.rs:4077-4119`) can produce `OperatorStatementOperand { place: None, ... }` when neither `canonical_place_at` nor `canonical_place_from_expression` resolves (`flow.rs:4094-4101`).

**Callers (read):** `rg 'apply_named_operator_call_effects' omega-rust` → 3 matches: definition and doc `flow.rs`, call `flow_expression.rs:11059` after named uses on a completed call expression. Same site calls `named_operator_call_mutated_places` at `flow_expression.rs:11072-11078` for the write timeline.

**Statement path (read):** `append_operator_statement_ensures` / statement mutations use `operator_statement_call_mutated_places` (`flow.rs:5553` region uses statement operand path).

**Input (read):** A `LocalData`/expression-form named operator call with a mutable parameter whose expression has no canonical place; expression effects skip pre-ensures invalidation and still run `append_operator_ensures_context`, while the parallel write-timeline helper may report unknown writes only if invoked separately.

**Verdict:** REACHABLE — facts that should be retired before operator `ensures` can remain active because the expression path neither invalidates nor marks writes unknown the way the statement path does.

## A0115

**Cited site (read):** In `append_operator_ensures_context` (`flow.rs:4349+`), for `ProofFact::Expression`, `dependencies` collects `operand.place.clone()` from operands (`flow.rs:4426-4429`). When `dependencies.is_empty()` (`flow.rs:4432`), the fact is appended with `place: FactPlace::Unknown` (`flow.rs:4433-4448`).

**Invalidation (read):** `visit_context_invalidations` (`flow.rs:7043-7072`): `fact.place.as_place()` on `Unknown` hits `else` at `flow.rs:7061-7065`; only `StorageDependency` payload sets `invalidated_any`; ordinary contract payloads with `FactPlace::Unknown` are not matched against `mutated_places`.

**Callers (read):** `append_operator_ensures_context` is invoked from `apply_named_operator_call_effects` (`flow.rs:4208`) and `append_operator_statement_ensures` (`flow.rs:4327`). Expression path: `flow_expression.rs:11059`; statement path: `flow.rs:5553`.

**Input (read):** A named operator `ensures` boolean expression where every operand lacks a resolved `CanonicalPlace` (all `place: None` in `named_operator_call_operands` / `operator_statement_operands`), so the ensures fact is published at `FactPlace::Unknown` and survives later statement writes.

**Verdict:** REACHABLE — operator ensures facts can persist across mutations that would invalidate the same payload if tied to a concrete place.

## A0116

**Cited site (read):** `state::build_state_flow_fact` (`flow.rs:4641-4656`). First action is `borrow.first_state_borrow(machine.symbol, state.symbol)`; `else { return; }` at `flow.rs:4652-4655` exits before `FlowStateFact` insertion (the insert at `flow.rs:4890+` is never reached).

**Callers (read):** `rg 'build_state_flow_fact\(' omega-rust` → 2 matches in `flow.rs`: definition `:4641`, sole call `builder::sweep_machine_states` at `flow.rs:667-677` inside `build_flow_facts` group sweep.

**Downstream (read):** Many checks use `facts.flow.control.state(machine, state)` and skip or `continue` on `None` (for example `checks_multiplicity.rs:2541`, `execution_terminal_unit.rs:773`, `checks_termination_progress.rs:4217`); no alternate flow row supplier was read for the same `(machine, state)` key.

**Input (inferred, not proved):** A `typed_trees` state that participates in checking or execution planning but has no corresponding `BorrowFacts` state row, so flow construction omits `FlowStateFact` entirely.

**Verdict:** UNDECIDED — the early return is definite at the site, but whether any program accepted through normal `build_borrow_facts` → `build_flow_facts` pipeline can lack `first_state_borrow` for a state that still must be checked was not established.

## What I did not read

- Operational plan construction to prove a suspending state can be missing from `OperationalPlan` while the machine still checks (`flow-effects` builders, corpus witnesses).
- Full storage-join consumer semantics for empty `Join` predecessor spans (`at_snapshot` walk, multiplicity cleanup uses).
- Complete `receiver_places` → contract proof obligation chain beyond `checks_contracts_prover.rs:6598`.
- All consumers of exit-rebased facts with invalid `FlowExitParameterOrigin` targets (`complete == false` paths in later `published_facts` / checks).
- Every `conditions.possible` caller in `checks_content.rs:4909` and full case-membership proof strategy.
- Whether `build_borrow_facts` always emits a borrow state row for every machine state that `build_flow_facts` sweeps.
- Corpus programs exercising expression-form named operator calls with unplaceable mutable operands.

SLICE COMPLETE
