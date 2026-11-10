slice_0015

| entry id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0109 | flow.rs:1259-1353 | CLOSED | Operational inference lists every program state; admission checks machine suspension contracts before summaries attach. |
| A0110 | flow.rs:1457-1539 | CLOSED | Non-static store targets skip snapshot rows by design; statement operators treat unknown writes as conservative; empty storage joins not seen on call paths. |
| A0111 | flow.rs:1605-1791 | UNDECIDED | `Some(empty)` vs `None` is clear at site; whether contract proving treats empty receiver sets as success was not fully traced. |
| A0112 | flow.rs:2604-2782 | REACHABLE | Exit rebasing always publishes contexts (`assumptions == false`), so entry-root contract facts can ship with unrebased parameter roots. |
| A0113 | flow.rs:3687-3734; checks_multiplicity.rs:6568; checks_contracts.rs:5760 | CLOSED | `possible == false` for unresolvable selections makes callers skip optimistic branches, not accept extra proof. |
| A0114 | flow.rs:4133-4221; flow.rs:4273-4296; flow.rs:5504-5521 | REACHABLE | Expression-form operators can publish ensures without invalidating when a mutable operand has no place; statement form clears contexts instead. |
| A0115 | flow.rs:4432-4449; flow.rs:7056-7071 | UNDECIDED | `FactPlace::Unknown` ensures are write-immune in invalidation; downstream contract checks for operator-only operands were not fully read. |
| A0116 | flow.rs:4652-4656 | CLOSED | Every state gets a borrow row in `append_state_borrow_facts` before flow runs, so the early return is unreachable on admitted programs. |

### A0109

1. Read `reach::attach_reach_summaries` in full at flow.rs:1259-1356 and `Default` on `SuspensionSummary` / related types at language-semantics/src/lib.rs:2047-2063 (derive `Default`, `direct_may_suspend: false`).
2. Search `\battach_reach_summaries\b` in omega-rust: 5 hits (flow.rs definition, re-export, `build_flow_facts` call at flow.rs:349, `finish_check_facts` at published_facts.rs:11231).
3. Read callers: `build_flow_facts` (flow.rs:291-350) runs after `build_groups`; `finish_check_facts` (published_facts.rs:11228-11236) runs `admit_prepared_program_effects`, then `validate_call_acknowledgements`, then `attach_reach_summaries`.
4. Read `infer_operational_may_for_dependencies` (validation/effect_inference.rs:1168-1179): one operational state row per `program.machine_states` entry.
5. Input reaching defaults: missing `operational_state` / `operational_call` `find` match ⇒ `unwrap_or_default()` on service reach, suspension, blocking, acknowledgement (flow.rs:1273, 1293, 1307, 1326, 1344, 1350-1353).

**Verdict: CLOSED.** Machine-level suspension/blocking mismatches are rejected at validation/machine_calls_core.rs:5422-5435 (`validate_behavior_plan`) before facts finish. Operational plans include every typed state (effect_inference.rs:1168-1179), so state-level default suspension is not taken for normal admitted bodies. Call-level defaults apply only when a flow call row fails to match an operational call record; acknowledgement validation at checking.rs:345-360 runs before the second attach. I did not read every operational call matching key variant.

### A0110

1. Read `append_store`, `append_preserved_binding`, `join` in full at flow.rs:1457-1540; `append_place_writes` at flow.rs:1419-1454.
2. Search `append_store(` in stage-05 crate: hits in flow.rs and flow_expression.rs (read sites at flow_expression.rs:4981-4989, flow.rs:5637).
3. Search `stored_values::join`: only flow_expression.rs:10393 (read `join_storage_prefixes` at flow_expression.rs:10387-10394 and boolean join at 175-184).
4. Read statement operator unknown path at flow.rs:5504-5521 (`operator_statement_call_mutated_places` returning `None` clears `active_contexts`).

**Input:** stores whose `destination.segments` fail `place_segment_has_static_identity` (flow.rs:1464-1469); join with predecessor list built from prefix snapshots.

**Verdict: CLOSED.** Skipping non-static-identity stores keeps the prior snapshot intentionally; statement operators treat unresolvable mutable places as unknown by clearing contexts (flow.rs:5514-5521). I found no production caller passing an empty predecessor slice into `stored_values::join` (only two-prefix boolean joins). Whether a zero-predecessor join node is reachable from `flow_state_values`’ unrelated `join` was not read.

### A0111

1. Read `captured_before_snapshot`, `reference_places_at_snapshot`, `before_snapshot`, `address_before_statement` at flow.rs:1582-1792.
2. Search `\baddress_before_statement\b` in omega-rust: 2 hits (definition; checks_contracts_prover.rs:6598).
3. Read `receiver_places` in checks_contracts_prover.rs:6574-6607 (uses `?` on `address_before_statement` only when the whole function returns `None`, not on empty `Vec`).

**Input:** `StoredOperand::Unavailable` and failed branches in `reference_places_at_snapshot` yield `Some(Vec::new())` (flow.rs:1647-1650, 1664-1666, 1674-1678), wrapped as `Some` by `before_snapshot` (flow.rs:1589-1594).

**Verdict: UNDECIDED.** At flow.rs:1773-1775, `before_snapshot` returning `Some([])` leaves `replaced == false` and still appends `source` to `output` (flow.rs:1787-1788), so “address vanishes” is not shown for that path in the function body I read. Whether prover or multiplicity checks treat `Some([])` from `receiver_places` as success was not traced past checks_contracts_prover.rs:6598.

### A0112

1. Read `rebase_contexts` in full at flow.rs:2604-2783 (especially 2740-2775).
2. Search `\brebase_contexts\b` in stage-05: 3 hits (flow.rs definition, flow.rs:4688 with `assumptions: true`, flow_exits.rs:1502 with `assumptions: false` — read exit caller).
3. Read exit caller context at flow_exits.rs:1502-1511.

**Input:** exit path (`assumptions == false`): machine-scoped contract facts whose `place.root` is a `Symbol` with no row in `origins` and no `invocation_referent` (flow.rs:2724-2735) keep unrebased root; `target` for `FlowExitParameterOrigin` is `unwrap_or_default()` when missing (flow.rs:2744-2747).

**Verdict: REACHABLE.** Condition `if complete || !assumptions` (flow.rs:2775) is always true on exits because `!assumptions` is true, so contexts publish even when `complete` is false after invalid `target` (flow.rs:2759). Smallest harmful program shape: a machine exit carries an entry-parameter ensures clause tied to a symbol that has no solved origin in `build.entry_origin_chain_at`, so the rebased fact table still records the entry parameter root at the exit point.

### A0113

1. Read `SelectionConditions::covers` and `possible` at flow.rs:3680-3749; `first_selection` at flow.rs:3800-3816.
2. Search `.possible(` in stage-05: 8 hits (flow.rs:3746 recursive; checks_multiplicity.rs:6568, 6592; checks_contracts.rs:5760, 6013, 6091, 6146; checks_content.rs:4909).
3. Read checks_multiplicity.rs:6566-6570 and checks_contracts.rs:5756-5762.

**Input:** `first_selection(condition)` is `None` for constant conditions or unresolved selection shape ⇒ `possible` returns `false` (flow.rs:3729-3730).

**Verdict: CLOSED.** Multiplicity uses `possible` only to error when overlap is possible (checks_multiplicity.rs:6568 — `false` suppresses the error). Contracts skip predicate materialization when `!possible` (checks_contracts.rs:5760 — `continue`). Unresolvable selections therefore weaken checks, not strengthen acceptance. I did not read checks_content.rs:4909.

### A0114

1. Read `apply_named_operator_call_effects` and `named_operator_call_mutated_places` at flow.rs:4133-4265; statement counterparts at flow.rs:4273-4296 and flow.rs:5504-5521.
2. Search `\bapply_named_operator_call_effects\b`: 3 hits (definition; comment; flow_expression.rs:11059 — read caller).
3. Read expression caller at flow_expression.rs:11058-11084 and `record_operand_writes` at flow_expression.rs:10357-10384.

**Input:** expression path returns early when operator or operand list cannot be rebuilt (flow.rs:4153-4164); otherwise drops mutable operands with `place: None` via `filter_map` / `flatten` (flow.rs:4166-4174) without invalidating when the list is empty. Statement path returns `None` from `collect()` on missing places (flow.rs:4292-4296) and clears contexts (flow.rs:5514-5521).

**Verdict: REACHABLE.** A named operator call inside an expression with a resolved operator but a mutable actual that has no canonical place can still run `append_operator_ensures_context` (flow.rs:4208-4221) while leaving pre-call semantic contexts intact; the statement form would clear contexts. Early return on full rebuild failure was not classified as accepting harmful programs (no ensures published either).

### A0115

1. Read `append_operator_ensures_context` at flow.rs:4349-4449 (`dependencies.is_empty()` branch at 4432-4434).
2. Read invalidation helper at flow.rs:7056-7072 (`fact.place.as_place()` else branch).
3. Search `FactPlace::Unknown` in stage-05: multiple publishers; did not read all contract consumers.

**Input:** operator ensures expression facts with no operand places publish at `FactPlace::Unknown` (flow.rs:4432-4434).

**Verdict: UNDECIDED.** Write invalidation skips non-place facts except `StorageDependency` (flow.rs:7061-7065), so ensures at `Unknown` survive mutations. Whether checks still require those ensures to hold without place linkage was not read in checks_contracts.rs beyond the `possible` gate.

### A0116

1. Read `build_state_flow_fact` entry guard at flow.rs:4652-4656 and body through `FlowStateFact` append at flow.rs:4866-4878.
2. Search `\bbuild_state_flow_fact\b` in omega-rust: 8 hits; sole production driver is the machine loop at flow.rs:667-677 (read).
3. Read `append_state_borrow_facts` tail at borrow.rs:8799-8809 (always appends one `StateBorrowFact` per state).

**Input:** `borrow.first_state_borrow(machine.symbol, state.symbol)` on states visited by `build_state_flow_fact`.

**Verdict: CLOSED.** Borrow construction always appends a state row before flow (borrow.rs:8799-8808); flow runs only after borrow facts build succeeds (published_facts pipeline). The guard is dead on admitted programs; missing flow state would surface as `None` from `flow.control.state` helpers, not silent acceptance. I did not read every downstream `expect` on flow state presence.

### What I did not read

- Full `validate_call` / nested-expression suspension diagnostics in checking.rs beyond skimming.
- checks_content.rs `possible` consumer and full contract proof paths for `FactPlace::Unknown` ensures.
- All `append_store` dynamic-index consumers outside the cited store timeline paths.
- `place_segment_has_static_identity` definition (re-exported from flow_place.rs; comment on fixed segments only was read).

SLICE COMPLETE
