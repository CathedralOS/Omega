Second pass for unit 59

Ranges covered (gap_016):
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs` lines 1201-7200 (6000 lines)

Path prefix below: `flow.rs` = `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/flow.rs` (checkout worker-23). Sibling files are in the same directory.

Where the range starts: line 1201 is inside `normal_returns::refresh` (the `mod normal_returns` that began at flow.rs:910), in the `let PlaceRoot::InvocationReferent { parameter, type_reference } = origin.root` destructure (1198-1201). Where it ends: line 7200 is inside `domain::invalidation::context_survival` (7184-...), `mod domain` starts at 6314 and its `mod invalidation` at 6880. I read lines 1170-1200 as header context (not counted).

Contradiction with earlier notes: unit_059.md:158 says `refresh` is at 1124 and was "read to line 1200, then cut off"; consistent. unit_059.md:173 lists `machine_groups` skip as "open"; not in my range, no comment. No contradiction found.

---

## Module map for 1201-7200 (read)

| Lines | Module | Kind |
|---|---|---|
| 1201-1249 | tail of `normal_returns::refresh` | inline |
| 1250-1357 | `reach` | inline |
| 1358-1359 | `state_values` (`#[path = "flow_state_values.rs"]`) | external file |
| 1360-1558 | `pub(crate) mod stored_values` | inline |
| 1559-1793 | `pub(crate) mod stored_views` | inline |
| 1795 | `pub(crate) use state_values::literal` | re-export |
| 1798-2860 | `entry_origins` (+ nested `invocation_referents` 1826-2122) | inline |
| 2865-2868 | re-exports from `entry_origins` | re-export |
| 2869-2870 | `exits` (`flow_exits.rs`) | external |
| 2871-3471 | `guard_predicates` | inline |
| 3472-3474 | re-exports | |
| 3475-3483 | `expression` (`flow_expression.rs`), `observations` (`flow_observations.rs`) + re-exports | external |
| 3484-3533 | `captured_references` | inline |
| 3534-3537 | `operand_contexts`, `operand_values` | external |
| 3538-3871 | `selection_conditions` | inline |
| 3872-3878 | re-exports | |
| 3879-4607 | `operator_calls` | inline |
| 4608-4615 | re-exports from `operand_contexts` | |
| 4616-4903 | `state` | inline |
| 4904-5782 | `statements` | inline |
| 5783-5837 | `storage_footprints` | inline |
| 5838-5839 | `transfers` (`flow_transfers.rs`) | external |
| 5841-5938 | `boundaries` | inline |
| 5939-5940 | `call_phases` (`flow_call_phases.rs`) | external |
| 5941-6016 | `call_results` | inline |
| 6017-6023 | `calls` (`flow_calls.rs`), `retained_borrows` (`flow_retained_borrows.rs`) + re-export | external |
| 6025-6313 | `borrow_lifetimes` | inline |
| 6314-7200+ | `domain` (`dependencies` 6321-6879, `invalidation` 6880-...) | inline |

---

## 1. `normal_returns::refresh` tail (1201-1249)

1. Purpose: the last part of building the per-machine-position "normal return predicates" roster. For each exit of a machine it finds which byte-sequence predicates are proved about an exclusive reference parameter (identified by its invocation referent), then keeps only the predicates every exit proves, and replaces the roster with them.
2. Shape: loop body ends by pushing `ReturnedPredicate { parameter, reference_type, predicate }` per `ByteSequencePredicate::ALL` where `transfers::carrier_proves_predicate` holds (1219-1229); `proved` is sorted by `ReturnedPredicate::key` and deduped (1232-1233); `common` is the intersection across exits (1234-1241); a non-empty `common` becomes a row `(position, common)` (1243-1245); `build.normal_return_predicates.replace(rows)` (1247).
3. Inputs/outputs: reads exit facts, `entry_origins::project_invocation_place` (1184, exact = true), `validation::reference_shell`. Writes `build.normal_return_predicates`.
4. How it decides: a fact is kept only if its place canonicalizes (1175-1183), projects to exactly one origin (1195), whose root is an `InvocationReferent` (1198-1201), has no segments, and whose type is an exclusive reference shell (1205-1208). Keyed on root identity (parameter symbol + type reference), not spelling. Predicates are keyed on `ByteSequencePredicate` enum value.
5. Cases by shape: "exactly one origin" `let [origin] = origins.as_slice() else continue` (1195-1197); only whole-referent (empty segments) exclusive-reference roots.
6. Same job elsewhere: none seen in my range.
8. Stated limits: none in range.
9. Fail-open: every `continue` (1173, 1183, 1194, 1197, 1203, 1209) drops a fact, so fewer predicates are published; the intersection (1235-1239) only shrinks. A missing row means "nothing proved" (per unit_059 notes 984-1029). none seen.

---

## 2. `reach` (1250-1357)

1. Purpose: copies service-reach, suspension and blocking summaries from the service-reach inference plan and the operational plan onto each finished state fact and each call fact, plus each call's operational acknowledgement (header 1251-1255).
2. Shape: single function `attach_reach_summaries` (1259) with `states.for_each_mut` (1266) and an inner loop over `calls.span_mut_or_empty(state.calls)` (1309).
3. Inputs/outputs: reads `flow_effects::ServiceReachInferencePlan`, `flow_effects::OperationalPlan`, `program.machine_by_symbol`/`machine_states`. Mutates `state.service_reach/suspension/blocking` and `call.service_reach/suspension/blocking/operational_acknowledgement`.
4. How it decides: matches by symbol and ordinal. State: `service_reaches.for_state(state.state_symbol)` (1267); operational state by `summary.symbol == state.state_symbol` scanning every machine (1275-1279, linear scan across all machines per state). Call: `statement_index` + `call_ordinal` + target state (1316-1318, 1334-1336). Target state is the callee machine's first state symbol, falling back to `call.target_symbol` (1310-1313).
5. Cases by shape: none.
6. Same job elsewhere: state-vs-call summary matching by `(statement_index, call_ordinal, target)` is the same rule used for the two plans at 1314-1320 and 1328-1338.
7. One-caller: `attach_reach_summaries` is `pub(crate)`; I did not grep it (not measured).
9. Fail-open: when a summary is absent, every field becomes `unwrap_or_default()` (1273, 1293, 1307, 1326, 1344, 1350, 1353). I did not read the `Default` impls of `ServiceReachSummary`/`SuspensionSummary`/`BlockingSummary`/acknowledgement, so whether default means "none" (permissive: a state that may suspend or block reports "does not") is **open**. A state or call with no row in the plan therefore gets the default summary; wrong program that could pass: one whose state is missing from the operational plan but does suspend.

---

## 3. `stored_values` (1360-1558)

1. Purpose: builds the chain of storage snapshots used to associate resident storage with the exact completed producer value. Header (1361-1367): "This is a value association, not permission to copy, retain a loan, or grant custody"; "Missing rows and unknown writes never fall back to a historical initializer."
2. Shape (all `pub(crate)` unless noted): `append_state_entry` (1383, empty writes snapshot with `previous = invalid`), `append_writes` (1397), `append_place_writes` (1419), `append_store` (1457), `append_preserved_binding` (1495), `join` (1524), `invalidates_destination` (`pub(in crate::flow)`, 1542). Re-exports `StoredObservation, StoredOperand, at_snapshot` from `validation::completed_storage` (1376).
3. Inputs/outputs: mutate `FlowControlFacts` arenas `storage_snapshots`, `operand_referent_segments`, `operand_origin_writes`, `operand_retained_paths`, `storage_predecessors`. Returns `Handle<FlowStorageSnapshot>`.
4. How it decides:
   - `append_writes` returns `previous` unchanged when both span lists are empty and both complete flags are true (1405-1407).
   - `append_place_writes`: `Option` places: `None` means "incomplete" (`bindings.is_some()` passed as `*_complete`, 1452-1453). A `None` list becomes an empty span with complete = false.
   - `append_store` and `append_preserved_binding` return `previous` (no new snapshot) if any destination segment lacks a static identity (`place_segment_has_static_identity`, 1464-1470, 1501-1507).
   - `join`: if all predecessors are the same handle returns it (1528-1532), otherwise a `Join` node (1536-1539). An empty list creates a `Join` with an empty span (no early return when `predecessors.first()` is `None`).
   - `invalidates_destination` (1542): `writes.is_none_or(...)` : unknown write list invalidates (fail-closed); else any write with same root and `operand_contexts::binding_overwrite` (1547-1556).
5. Cases by shape: `append_store`/`append_preserved_binding` accept only destinations whose every segment has static identity (1464-1470).
6. Same job elsewhere: `append_store` and `append_preserved_binding` repeat the same destination-to-`FlowOperandReferent` construction (1471-1488 vs 1508-1521); `append_place_writes::rows` (1425-1444) builds `FlowOperandReferent` a third time.
7. Caller counts (grep `stored_values::(...)` and bare names across `src/`): `append_state_entry` 3 (flow.rs:5008, flow_exits.rs:1457, plus definition); `append_place_writes` used flow.rs:4921 import and flow_expression.rs:10378; `append_store` flow.rs:4922, flow_expression.rs:4838; `join` flow_expression.rs:4839/5002/10393; `invalidates_destination` flow_call_phases.rs:1335 import, flow_expression.rs:4627; `append_preserved_binding` 1 outside user (flow_expression.rs:4991 via import 4837); `append_writes` 1 call, inside this file (flow.rs:1447), pub(crate) with no outside caller (grep `append_writes\(` in `src/`).
8. Stated limits: "Missing rows and unknown writes never fall back to a historical initializer" (1365-1366).
9. Fail-open: `append_store` / `append_preserved_binding` returning `previous` when a segment is not static-identity (1469, 1506): the store is not recorded, so a later query sees the older snapshot. That is a missing-store, i.e. the stored value is unknown rather than wrong; readers consult `at_snapshot` (not read). "open" whether an older snapshot could still answer a query for the destination with the pre-store value. `join` with empty predecessors: "open", not traced.

---

## 4. `stored_views` (1559-1793)

1. Purpose: answers "which exact places does this reference/descriptor point at, at this storage snapshot", using completed stores. Header (1560-1563): the same query serves operand evaluation and later effect proofs and "neither may replay an initializer or replace an unresolved newer descriptor with a historical source address."
2. Shape: `before_snapshot` (1582), `captured_before_snapshot` (1597), `reference_places_at_snapshot` (1634), `capture_at_snapshot` (1689), `address_before_statement` (1730).
3. Inputs/outputs: reads `FlowControlFacts`, `State`, `CheckingInput`, `validation::completed_storage::{reference_sources_at_path, at_snapshot}`. Returns `Option<Vec<CanonicalPlace>>` or `Option<Vec<CapturedReferencePlace>>`.
4. How it decides:
   - `capture_at_snapshot` (1689): returns `None` when the source is a whole state parameter whose reference shell is not (Shared and mutable) (1696-1708); otherwise `at_snapshot(..., StoredObservation::Binding)`; else for a local whose type `operand_binds_live_view`, returns `StoredOperand::Unavailable` (1718-1723).
   - `reference_places_at_snapshot` (1634): pops a work list; `Unavailable` => returns `Some(Vec::new())` (empty = give up, 1647); `Incoming(place)` kept only if `incoming_is_admitted(&place)` else empty (1648-1651); `Alternatives` expanded; `Value` requires a valid capture and non-empty selection else empty (1664-1679).
   - `captured_before_snapshot` (1597): no captures => `Some(Vec::new())` (1605-1607); any empty selection => `Some(Vec::new())` (1617-1622). The `before_snapshot` shim drops formation (1582-1595).
   - `address_before_statement` (1730): finds the last statement before `before` in admitted execution order (1739-1755), then rewrites place prefixes that are live views until none change; depth limit 64 => `None` (1759-1761).
5. Cases by shape: depth bound `depth >= 64` (1759); for each prefix length, only prefixes whose type `operand_binds_live_view` are rewritten (1770); a single result equal to the prefix is treated as unchanged (1777).
6. Same job elsewhere: both `reference_places_at_snapshot` (1634) and `captured_before_snapshot` (1597) call `captured_operand_reference_places_with_formation` / `reference_places_at_snapshot`; `before_snapshot` is a `.place` projection of `captured_before_snapshot` (1589-1594).
7. Caller counts: `address_before_statement` 1 outside caller (checks_contracts_prover.rs:6598). `captured_before_snapshot` 1 call site, in this file (flow.rs:1590) though `pub(crate)` (grep `captured_before_snapshot\(` in `src/`). `before_snapshot` 1 call site, in this file (flow.rs:1773), `pub(crate)`. `reference_places_at_snapshot` outside callers: flow_expression.rs:396 and imports at 6260. `capture_at_snapshot`: imports at flow_expression.rs:6259, 7936.
8. Stated limits: `address_before_statement` doc (1726-1729): "Resolve only descriptor crossings on an address."
9. Fail-open: an unresolved descriptor returns an **empty** Vec, with `Some(...)` not `None` (1647, 1665, 1676, 1678, 1618, 1621). Callers that treat `Some(empty)` as "points at nothing / no aliases" would be permissive; `address_before_statement` treats `places.len()==1 && == prefix` as no change but an empty result `places` makes `replaced = true` with nothing pushed (1780-1784), so the address vanishes from `output`. Whether that means "no storage touched" downstream is **open** (not traced into checks_contracts_prover.rs:6598).

---

## 5. `entry_origins` (1798-2860)

1. Purpose: tracks, across named-state transitions of one machine, which entry parameter each state parameter came from (values and reference addresses), solved to a fixed point; and rebases semantic contexts/facts from entry-parameter names to the state-parameter names that carry them. Header (1799-1805): a normal-return requirement can name the original invocation referent after its runtime binding disappears; "not a forwarded state parameter".
2. Shape:
   - `invocation_referents` (1826): `private_owned_projection` (1847), `invocation_referent` (1886), `nonresident_content_region` (1906), `project_invocation_place` (1981), `extend_invocation_observations` (2052).
   - helpers: `reference_type` (2128), `immutable_value` (2139), `tracked_parameter` (2148), `stable_parameter` (2155).
   - `ParameterOrigins { parameter, sources: Vec<SymbolHandle>, storage: Vec<Option<LocalWriteSource>> }` (2175), `ParameterEdge` (2181), `tracked_source_parameter` (2196), `parameter_edges` (2237), `MachineParameterOrigins { rows, storage_available }` (2327; `covers` 2337), `machine_parameter_origins` (2346), `activation_parameter_origins` (2358), `machine_storage_origins` (2370), `solve_parameter_origins` (2378), `root_storage` (2498), `transport_storage` (2515), `entry_storage_origins` (2561), `state_origins` (2581), `rebase_contexts` (2604), `append_immutable_exit_origins` (2804).
3. Inputs/outputs: reads machine/state tables, `CallFrameResolver` (`transition_write_origins`, `proof_value_is_caller_isolated`, `storage_origin_substitutions_are_finite`), `FactPlan`, `FlowBuildContext`. Produces `MachineParameterOrigins`, projected `CanonicalPlace`s, rebased context spans, `FlowExitParameterOrigin` rows (in `build.control.exit_parameter_origins`).
4. How it decides:
   - Parameter tracking: a parameter is tracked iff its type is a reference (peeling `Constrained`) or an immutable non-const non-reference value (2128-2153). `reference_type` peels only `Constrained` (2134).
   - Stability: immutable value, or `state_reference_parameter_binding_is_stable` (2155-2167).
   - Edge building (2237-2319): only `TransitionExit::Ordinary` transitions with `Named` targets whose `destination_index` resolves (2258-2269). An argument maps to a source parameter only if it is a single-member name path, following immutable locals' initializers (2196-2229); a mutable local is opaque; the source parameter must be both-references or both-immutable-values (2221-2224); then it must be `stable_parameter` else mapped to an invalid symbol = unknown origin (2301-2306).
   - Fixed point (2431-2491): origins are unioned, "never chosen by predecessor order" (2172); unknown = invalid symbol (2473) vs empty set = not yet reached (2170-2171). Only edges from reachable states propagate (2434-2436).
   - `project_invocation_place` (1981): rebases to an invocation referent; `exact` requires exactly one identical source and no coarse (collection_coarse / recasts / representation) (2015-2037); non-referent source requires `private_owned_projection` else `None` (2021-2029).
   - `private_owned_projection` (1847): every prefix type must have no reference shell and the selected type must be `proof_value_is_caller_isolated` (1870-1883).
   - `nonresident_content_region` (1906): all sources must have the same first recast being an `ExtentView` formation with no representation, same root, static segments, authenticated declaration equal to itself, and operation `View` (1936-1958); unequal regions => `None` (1968-1970).
   - `rebase_contexts` (2604): facts whose point is not `ProgramPoint::Machine` for this machine are passed through unchanged (2676-2685, 2697-2701); others are re-pointed (`scoped_point`, 2686-2693) and their roots rewritten via `origins` lookup (2726-2735).
5. Cases by shape:
   - `extend_invocation_observations` unknown fallback `field_path` (2087-2093): segments only `Field`/`Case` with valid symbols or `FixedIndex`; anything else collapses to the whole referent (2101-2105). Quote: `_ => false`.
   - `private_owned_projection` returns false if any prefix has a reference shell.
   - `nonresident_content_region` requires `ReferenceViewFormation::ExtentView` and `AccessLifecycleOperation::View` (1938-1957).
   - `transport_storage` (2515): by shape of source: has `invocation_result` (kept), invalid root (None), root is a tracked parameter row (substituted), local non-reference (kept), local reference (None), non-reference parameter (kept), else None (2521-2553).
6. Same job elsewhere: `project_invocation_place` (1981) and `nonresident_content_region` (1906) share the same prelude (rebase_exact_local_place, stable-reference-parameter check, `storage_origins_at`, `entry_storage_origins`): lines 1914-1934 vs 1994-2014 (duplicated by reading). `machine_parameter_origins` (2346), `activation_parameter_origins` (2358) and `machine_storage_origins` (2370) are three wrappers over one `solve_parameter_origins` (2378) differing in (frames, destination_index).
7. Caller counts (grep count mode, whole `omega-rust`): `append_call_boundary_edges|invocation_referent\(|project_invocation_place|extend_invocation_observations|nonresident_content_region|rebase_contexts` = 30 occurrences, 20 in flow.rs, rest in flow_state_values.rs (2), flow_place.rs (1), flow_expression.rs (1), flow_exits.rs (1), flow_call_phases.rs (3), flow_calls.rs (2); per-name not separated. `activation_parameter_origins`: 4 outside calls, all checks_contracts_exits.rs (798, 987, 1250, 1409). `MachineParameterOrigins::covers`: 1 outside caller (flow_context.rs:823). `entry_storage_origins|machine_parameter_origins|machine_storage_origins|state_parameter_origins` also used elsewhere (grep count over those and `covers(`: 60 occurrences in 14 files; per-name not separated). Header 2861-2864 says contract induction "consumes the same all-path origin solution as flow rebasing": `activation_parameter_origins` is used by checks_contracts_exits.rs 4 times (read by grep), so that claim has outside users.
8. Stated limits: "Unknown/opaque carriers cannot supply that proof" (1845); "a recast's logical selectors are not declaration fields, so retain only its enclosing owner for may-write observations and decline it for exact proof" (1977-1980); "mutable local is opaque" (2191-2192).
9. Fail-open:
   - `solve_parameter_origins` (2473): a source row not found maps to `vec![SymbolHandle::invalid()]` = unknown origin, so it is conservative. none seen.
   - `rebase_contexts` (2741-2743): `if !program.state_parameter_by_symbol(entry, root).is_some() { continue; }` : a required root that is not an entry parameter is skipped and does not clear `complete`. With `assumptions` false, `complete &= target.is_valid() && !assumptions` is always false anyway (2759) and the context is appended regardless (`complete || !assumptions`, 2775). So for non-assumption rebase (exit contexts) a fact whose root has no valid target origin is still published with its original (unrebased) root in the context at 2772-2773. What wrong program could pass: a normal-return fact about an entry parameter that no longer maps to a state parameter is kept un-renamed. "open": I did not trace how `FlowExitParameterOrigin` rows (written only when `contract` is Some, 2760-2770) gate use later.
   - `append_immutable_exit_origins` (2823-2848): `continue` on any mismatch, so no origin row; missing row = no proof (stated 2799-2803 "missing optional origins supply no proof and revive no storage"). stated.
   - `extend_invocation_observations` (2086-2112): on `project_invocation_place` None it conservatively adds every invocation referent (stated 2047-2050 "If projection is unknown, invalidate every possible incoming referent"). stated, fail-closed.
   - `private_owned_projection` with `build.call_frames` None returns false (1854-1856). fail-closed.

---

## 6. `guard_predicates` (2871-3471)

1. Purpose: derives which Boolean sub-expressions of a transition guard are known true/false when a given arm is taken, including "saved decisions" (a Boolean stored in a local/parameter earlier) provided the reads they depend on were not overwritten in between. Header (2872-2879): "Saved decisions are historical values, not promises about current storage"; "Generated names and adjacency grant no authority."
2. Shape: `GuardObservation { expression, value, statement_index, current_expression }` (2909; `evaluated` 2928), `transition_guard_observations` (2950, `pub(crate)`), `saved_guard_observations` (3031, `pub(crate)`), `reaching_capture` (3136), `reads_survive` (3204), `guard_observations` (3365, `pub(super)`), `expression_is_stable_predicate` (3440, `pub(super)`).
3. Inputs/outputs: reads expression/statement tables, `CallFrameResolver` write frames (`expression_write_frame`, `assignment_write_frame`, `may_write_frame`, `statement_value_write_frame`, `reference_expression_*`), `place_at`. Returns `Vec<(ExpressionHandle, bool)>` or `Vec<GuardObservation>`.
4. How it decides:
   - `guard_observations` (3365-3438): decomposes `!x`, `a && b` when true, `a || b` when false, `x == true/false` / `!=` comparisons with a Boolean literal, and `match` with a unique arm for the outcome (`match_result_unique_arm`, 3427-3433); else pushes the whole expression. Decomposing `!` and `==` is gated on `has_builtin_decomposed_guard_meaning` (3377-3382, 3399-3404): so a user-overloaded `!`/`==` is not decomposed.
   - `reads_survive` (3204): true if `writes` empty (3213); otherwise collects every read node and requires each to be a name/member/indexed/borrow whose place's frame coordinate path overlaps none of `writes` (3274-3286) and whose reference origins before the statement overlap none either (3316-3356). An atomic read fails (3270, 3306). Const names and exact-case-reference owners pass (3259-3265). Any node kind not Name/Member/Indexed/Borrow/Atomic returns true (`_ => return true`, 3271).
   - `reaching_capture` (3136): walks statements backward to the defining `LocalData` initializer or assignment to the symbol, accumulating write frames of statements in between; returns `None` on an uninitialized local (3163-3165), on a non-ordinary/continuation/always transition (3177-3182), or on any statement kind not in {Transition, Assignment, Call, LocalData, Expression} (`_ => return None`, 3198).
   - `expression_is_stable_predicate` (3440): the expression subtree must be only Name/literals/Struct/Array/Member/Indexed/Unary/Binary/Borrow nodes, or a `byte_predicate_call` (descends into its operand, 3463-3466); every other node kind makes it unstable (`_ => SubtreeVisit::Found`, 3467).
   - Keyed on expression handles, symbols and place frame paths. `SymbolKind::Local | Parameter` gate for saved subjects (3065-3072). Compares paths with `frame_paths_overlap` (string-path based: `writes: &[String]`, 3210, 3278).
5. Cases by shape: `reaching_capture` statement-kind whitelist (3175-3199, quoted above); `expression_is_stable_predicate` node-kind whitelist (3448-3468); `guard_observations` decomposition list (3375-3434); `saved_guard_observations` subject must be `single_segment_name_symbol` (3062).
6. Same job elsewhere: the arm target operand collection of `transition_guard_observations` (2969-2976) resembles `TransitionTargetNode` handling elsewhere (suspected, not read).
7. Caller counts (grep count `transition_guard_observations|saved_guard_observations|guard_observations\(|expression_is_stable_predicate|reads_survive`): 40 occurrences in 5 files: flow.rs 20, flow_exits.rs 7, checks_contracts_calls.rs 8, checks_contracts.rs 3, checks_ranges.rs 2. So the outside users are checks_* and flow_exits.rs; per-name not separated.
8. Stated limits: "Dispatch precedes arm operands: both direct predicates and saved decisions must name reads preserved through those operands" (2943-2946).
9. Fail-open:
   - `reads_survive` `_ => return true` (3271) and `else { return true; }` for non-reference, non-member, non-indexed nodes (3346-3348): an expression node kind other than Name/Member/Indexed/Borrow/Atomic is treated as having no stored read. Inputs that reach it: calls, binary nodes etc. appear in `nodes` only via `nodes.push(node)` when `reads_value` (3243) — Binary nodes are pushed and recursed via `children` (3252). So a Binary node itself returns true while its children are checked separately. "traced" (read 3216-3256): children are pushed to `pending` and examined, so this is not a gap for compound expressions. A `Call` node would also be pushed and its children visited; the call's own effects are not considered by `reads_survive` — "open": whether `expression_is_stable_predicate` (rejects Call except byte predicate calls, 3463) is always applied before, which it is for the two call sites at 2991-3001 and 3099-3108, so "traced" for those callers; the `GuardObservation` for `current_expression` is only set under that check (3099-3112).
   - `transition_guard_observations` returns `Some(Vec::new())` when the guard is not `When` (2959-2961): no observations; fail-closed.
   - `saved_guard_observations` returns an empty Vec when frames absent or write frames incomplete (3041-3049). fail-closed.
   - `reads_survive` `frame_coordinate_path(...).is_some_and(...)` gives false on unresolvable path (3275). fail-closed.

---

## 7. `captured_references` (3484-3533)

1. Purpose: scratch struct for a reference's geometry that keeps the exact descriptor-formation handle alongside the place (header 3485-3489).
2. Shape: `CapturedReferencePlace { place: CanonicalPlace, formation: Handle<FlowReferentFormation> }` (3496); `unformed` (3502), `from_referent` (3509), `projected` (3525, delegates to `super::project_captured_view_place`).
4. Decides: none. `from_referent` returns `None` if `operand_referent_segments.span(...)` is invalid (3516-3520).
7. Used in this range: `stored_views` (1603, 1641, 1652). Re-exported `pub(crate) use` (3533). Outside-user count not measured.

---

## 8. `selection_conditions` (3538-3871)

1. Purpose: a small hash-consed Boolean DAG over "which alternative was chosen" for saved value selections, with coverage ("every alternative of every choice is covered") and possibility queries computed by restricting one choice at a time and memoizing (header 3539-3548): "never materializes a Cartesian product ... or a DNF"; "scratch for consumers of already produced captures, not another evaluator".
2. Shape: `Condition` enum False/True/Arm(CapturedValueChoice)/All/Any (3558); `SelectionConditions` (3567; fields `nodes: Arena<Condition>`, `truth`, `restrictions`, `coverage`, `possibilities` as `Vec`s). Methods: `new` 3576, `always` 3588, `never` 3592 (returns `Handle::invalid()`), `choice` 3596, `choices` 3613, `all` 3626, `any` 3651, `covers` 3673, `possible` 3711, `negate` 3752, `intern` 3793, `first_selection` 3800, `restrict` 3819; free fn `ordered` 3861.
3. Inputs/outputs: reads `control.operand_alternatives` spans. Outputs condition handles / bools.
4. Decides: `never()` is the *invalid handle* and `Condition::False` is nodes' default variant, so "false" is represented by an invalid handle (3592-3594) while `negate` matches `*self.nodes.get(condition)` for False (3758) — `get` on an invalid handle is not guarded in `negate` (3757); `all`/`any`/`covers`/`possible`/`restrict` guard `is_valid`. "open": whether `negate(never())` can panic in the arena. Callers call `negate` on `bad` that starts as `never()` (checks_contracts.rs:5758, 6041, 6107, checks_content.rs:4902 per grep lines), so a handle that was never widened is passed to `negate`; I did not read `Arena::get` on an invalid handle.
   - `covers` (3673): empty alternatives rows are not covered (`!rows.is_empty()`, 3693); invalid handle => false (3681).
   - `choice` (3596): rejects when the span is empty or the `choice.alternative` is not one of its handles (3601-3609).
   - `all`: two distinct alternatives of the same selection => never (3640-3646).
   - `intern` (3793): linear search of the node arena (`nodes.iter().find`) — O(n) per intern.
   - Memo vectors are looked up by linear scan (3684, 3722, 3827-3832).
5. Cases by shape: none beyond node kinds.
6. Same job elsewhere: `covers` (3673) and `possible` (3711) are the same recursion with `all` vs `any` over alternatives.
7. Caller counts (grep over `omega-rust`): `SelectionConditions::new` called at checks_multiplicity_claim_outcomes.rs:619, checks_contracts.rs:6376, checks_content.rs:578, 1602, 6937; `.negate(` on this type at checks_contracts.rs:5758, 6011, 6041, 6107 and checks_content.rs:4902; `.possible(` in checks_multiplicity.rs:6568, 6592, checks_contracts.rs (several), checks_content.rs:4909; `.covers(` in checks_content.rs, checks_contracts.rs, checks_multiplicity.rs:6574; `.choices(` checks_contracts.rs:6379, checks_content.rs:4891. Every public method has at least one outside caller (read from grep output). `negate` of `bignum` types in the same grep are unrelated.
8. Stated limits: none.
9. Fail-open: `first_selection` returning `None` for a non-constant condition gives `covers` = false and `possible` = false (3687-3689, 3729-3731). `possible` false = "impossible" is fail-open for checks that treat an impossible overlap as "skip": checks_multiplicity.rs:6568/6592 and checks_contracts.rs:5760 call `possible(...)` and skip on false. A condition whose first selection span is not valid (`operand_alternatives.span(selection)` None) gives `possible = false` (3732-3734), so an overlap could be judged impossible. "open": not traced into those callers.

---

## 9. `operator_calls` (3879-4607)

1. Purpose: handles named calls that dispatch to an operator or boundary definition (which produce no borrow-call fact): resolve the operator, list the places its mutable operands write, and add its `ensures` onto the caller's operands. Header (3880-3885): "Statement calls and expression-form named uses share these rules."
2. Shape: `ResolvedOperatorStatementCall` (3915), `resolve_operator_statement_call` (3926), `resolve_operator_for_call` (3967), `OperatorStatementOperand` (3986), `operator_statement_operands` (3993), `named_operator_call_operands` (4077), `apply_named_operator_call_effects` (4133), `named_operator_call_mutated_places` (4236), `operator_statement_call_mutated_places` (4273), `append_operator_statement_ensures` (4306), `append_operator_ensures_context` (4349), `operator_contract_relative_place` (4551).
3. Inputs/outputs: reads `typed_trees::operator` (`resolve_named_call`, `declaration_by_symbol`, `operator_parameters`), `program.signature_contracts`, `program.proof_facts`. Appends `Fact`s with `FactOrigin::OperatorEnsures` to `FactPlan`; mutates active context/constraint spans, invalidations.
4. How it decides:
   - Receiver: `receiver_is_value = call.receiver_symbol.is_valid() && symbol_type_symbol(...).is_some()` (3930-3931); otherwise the receiver spelled path members are passed as static receiver segments (3932-3948), and `target_name` is `call.target.as_str()` (3953): resolution is via `typed_trees::operator::resolve_named_call` with name strings + arity (3975-3982). So resolution uses spelling of path members and the target name plus symbol.
   - `operator_contract_relative_place` (4551): matches parameter by `parameter.name.as_str() == head` (spelling) OR `head_symbol`/`symbol` (4566-4570); comment 4547-4550: "Contract name paths are intentionally not resolved as caller symbols, so the parameter's authored name is the fallback root." Spelling-keyed.
   - `append_operator_ensures_context` (4349): for `Ensures` contracts only (4370); `Membership` facts need a relative place rooted at a parameter and a matching operand with a canonical place (4376-4394), else `continue` (skipped); `Expression` facts instantiate a label from operand labels (`instantiate_operator_contract_expression_label`, 4419-4424) and publish one `ContractBooleanExpression` fact per operand place dependency, or a single `FactPlace::Unknown` one if no operand has a place (4432-4449); `Proposition` facts publish on `append_symbol_place(application.proposition)` (4475). Operand labels are display names (`program.expression_table.display_name`, 4054) or canonical-place labels (4031).
   - `dependencies.dedup()` (4430) removes only adjacent duplicates.
   - `operator_statement_call_mutated_places` (4273): `Some(Vec::new())` when no operator resolves ("an ordinary non-operator call contributes no additional named-call writes", 4271-4272); `None` if a mutable operand has no place (`collect::<Option<Vec<_>>>`, 4292-4296).
5. Cases by shape: `ProofFact::{Membership, Expression, Proposition}` match (4374-4519); `operator_contract_relative_place` accepts only Borrow/Name/Member/Indexed (4556-4605), else `None`; for Name requires every member symbol after the head to be a valid symbol else `None` (4575-4578).
6. Same job elsewhere: `operator_statement_operands` (3993) and `named_operator_call_operands` (4077) are the statement and expression forms of the same operand-binding job (stated in 4069-4076). `operator_statement_call_mutated_places` (4273) and `named_operator_call_mutated_places` (4236) likewise (stated 4234). `resolve_operator_statement_call` header 3920-3925 says "every consumer (ownership, mutation invalidation, and postcondition flow) must use this one resolution rule". Grep: `resolve_operator_statement_call` outside this module: flow_ownership.rs:1195, flow.rs:11750 (re-export) and 11788 (a wrapper). `resolve_operator_for_call`: flow_ownership.rs:1083 and 1398 plus 432 import. So the single rule has two outside users in flow_ownership.rs (read by grep, not read in full).
7. One-caller: `named_operator_call_mutated_places` 1 outside call (flow_expression.rs:11072); `apply_named_operator_call_effects` and `append_operator_statement_ensures` are used in this file/others (not grepped separately, "not measured").
8. Stated limits: "A missing mutable operand coordinate is unknown; an ordinary non-operator call contributes no additional named-call writes." (4270-4272); `None` means unknown and "must read it as an overlap with every earlier operand source, never as 'writes nothing'" (4231-4233).
9. Fail-open:
   - `apply_named_operator_call_effects` (4146-4165): returns early (no invalidation, no ensures) when the operator declaration or operands can't be reconstructed. Missing invalidation means no context filtered after a `&mut` operand write. Note `named_operator_call_mutated_places` returns `None` in the same case (4244-4259), which the timeline reads as unknown (stated 4231-4233), but this effect function simply returns. Which wrong program passes: an expression-form operator call whose operands cannot be rebuilt would not drop facts about its mutable operands in the active contexts. "open": whether the other use (flow_expression.rs:11072) compensates.
   - `apply_named_operator_call_effects` (4166-4174): a mutable operand with `place == None` is filtered out by `filter_map` and simply not invalidated (no fallback to "unknown write"). In contrast `operator_statement_call_mutated_places` (4292-4296) returns None for that case. This is an asymmetry between the expression and statement paths: "open" (not traced).
   - `append_operator_ensures_context` membership `continue`s (4378, 4383, 4388, 4392): ensures not published when the place cannot be rebuilt; fail-closed (fewer facts).
   - `FactPlace::Unknown` fact with `QualificationEvidence::default()` for expression ensures with no operand places (4432-4449): published as true-fact over no place. It cannot be invalidated by place-based filtering (`matching_mutation_for_fact_place` is only reached via `fact.place.as_place()`; `visit_context_invalidations` skips facts without a place unless `StorageDependency`, 7061-7066) — so it persists through every later write in that context. "traced" (read 7056-7071): such a fact is never invalidated by a write. It is an `ensures` that mentions no operand place, e.g. constant-only; "open" whether other expression kinds with only unplaceable operands (e.g. operands that are rvalues) end up here.

---

## 10. `state` (4616-4903)

1. Purpose: builds the flow fact for one state: collects declaration and state-entry contexts, rebases them, adds borrow constraints, materialises `requires` bounds, runs the statements, drops expired loans, records exit facts, appends the `FlowStateFact` (header 4617-4622).
2. Shape: single fn `build_state_flow_fact` (4641).
3. Inputs/outputs: reads `BorrowFacts`, `ProofFacts`, `DomainFacts`, `FactPlan`; writes many arenas in `FlowBuildContext` and appends `FlowStateFact` (4866-4901).
4. Decides: returns immediately (no row) if `borrow.first_state_borrow(machine.symbol, state.symbol)` is None (4652-4656). Machine `requires` apply only for the entry state (4745-4748, stated 4741-4744). Only `Requires` contracts with expression facts produce byte-predicate and comparison-bounds contexts (4767-4773).
5. Cases by shape: none.
6. Same job elsewhere: contexts-to-constraint-refs conversion at 4705-4718 is the same shape as other `append_flow_contexts*` helpers (not read).
8. Stated limits: comment 4662-4666: "All source-driven checker tests replay the former declaration lookup at this exact point."
9. Fail-open: `build_state_flow_fact` early return (4652-4656): a state with no borrow state fact gets **no** flow state row. Whether downstream consumers treat a missing state row as "nothing known" (fail-closed) or skip checks is **open** (not traced).

---

## 11. `statements` (4904-5782)

1. Purpose: per-state statement sweep. For each statement in admitted execution order it records the statement fact, runs transition arms or calls, computes write footprints, invalidates facts, activates borrow loans, appends operator ensures, propagates transfers and records the storage snapshot (header 4906-4911).
2. Shape: `append_state_statement_flow_facts` (4947), `append_proof_output_ensures` (5684), `unknown_alias_footprints` (5727), `statement_record_ranges` (5761).
3. Inputs/outputs: reads borrow call/loan tables; writes `FlowStatementFact` rows (5010-5027) and mutates `build.control.*`, `build.invalidations`, `build.borrow_lifetimes`, `active_contexts`, `active_constraints`.
4. How it decides:
   - Write footprint resolution order (5313-5388): assignment -> `storage_footprints::assignment` (the destination from completed operand capture); if `write` exists, the semantic target place (exact => `[target]`, else `candidate_targets`); `LocalData` => `Some(empty)`; otherwise `build.statement_storage_writes_at`. `None` here sets `completed_unknown_write = true` (5382-5388).
   - Content writes for an assignment: closes writes over local-write origins (`local_write_origins_before_store` + `close_storage_places_over_origins`), falling back to `unknown_alias_footprints` (5389-5435).
   - If `storage_writes` is `None`, all active contexts are dropped (`HandleSpan::empty()`, 5462-5470): unknown write ⇒ forget everything. Same for a `Call` whose operator mutated places are unknown (5504-5523).
   - `write.destination_is_current` computed via `captured_assignment_destination_remains_current` (5162-5190).
   - Loans: skip loans established by a call or descriptor-load observation (5262-5265); `formation_value` is the unique captured operand whose `expression == loan.formation_expression`, not an `InvocationResult`; if more than one, invalid (5269-5296).
   - `statement_record_ranges` (5761): asserts "one statement owns one contiguous borrow cohort" (5773-5776) and `expect`s that the borrow record names a valid frame (5767-5769): panics rather than error.
5. Cases by shape: statement-kind match for `final_write` (5582-5611): `Assignment | Assembly` get both write lists; `LocalData` gets a self-write snapshot; others `None`. Index-segment narrowing only for non-assignment statements with `PlaceSegment::Index` (5478-5503).
6. Same job elsewhere: `filter_contexts_after_place_mutations` + `project_constraint_refs_to_active_contexts` + `extend_invocation_observations` triple repeated at 5524-5549 and at operator_calls 4175-4206 (apply_named_operator_call_effects). Duplicate by reading.
7. One-caller: `append_state_statement_flow_facts` `pub(super)`, one caller in `state` (4830).
8. Stated limits: `unknown_alias_footprints` doc (5719-5726): "Opaque, recursive and unresolved types remain conservative. This excludes aliases; it grants no domain facts or exemption from the physical write footprint."
9. Fail-open:
   - `unknown_alias_footprints` (5727-5754): a local is skipped as non-alias when `frames.value_is_reference_free(local.type_reference)`; if `frames` is None (resolver unavailable) `frames.is_some_and(...)` is false so the local is included (conservative). fail-closed.
   - `filter_reassigned_borrow_loans` called with `reassigned_place = None` unless the write is exact AND `destination_is_current` (5198-5210): loan release on reassignment only on exact current destination; fail-closed (loans kept).
   - Loan-activation `continue` for `established_by_call.is_valid()` (5262): loans created by calls are activated elsewhere ("traced" only by name; call builder not read).
   - `append_proof_output_ensures` (5694-5698): returns when no proof-only output binding; otherwise appends a `CallEnsures` point context. none seen.

---

## 12. `storage_footprints` (5783-5837)

1. Purpose: gives the physical places an assignment may write, from the address captured before the RHS (header 5784-5790): "That enclosure bounds possible writes; it never establishes a stored value, equality or untouched contents."
2. Shape: `assignment` (5799, `pub(super)`).
4. Decides: invalid target capture => `None` (5807); captured `referents` non-empty => `direct_referent_places` (5811-5821); else needs call frames (`?`, 5823), `assignment_replaces_reference_binding(...)?` and if true => `None` (5824-5826); else `reference_backing_places(...)` with a closure that always returns `None` (`&mut |_, _| None`, 5827).
9. Fail-open: `None` means unknown write, which the caller (statements 5340-5341, 5382-5388) turns into "unknown write" (drops all active contexts). fail-closed. none seen.

---

## 13. `boundaries` (5841-5938)

1. Purpose: boundary edges for one call: for every boundary trait the callee's machine conforms to (directly or through trait requirements), one `FlowBoundaryEdgeFact` per trait signature whose name equals the called state's name.
2. Shape: `append_call_boundary_edges` (5853), `append_boundary_edges_for_trait` (5885, recursive over `trait_requirements`, with `visited_traits` push/pop).
4. Decides: keyed on **name equality**: `signature.name == target_state.name` (5908). Boundary-only: `trait_definition.is_boundary` (5904). Unresolved callee `state_location` => empty span (5858-5866).
5. Cases by shape: none.
6. Same job elsewhere: none seen.
7. Caller count: `append_call_boundary_edges` appears in the 30-occurrence grep set above (flow.rs 20, flow_calls.rs 2, others), per-name not separated, "not measured".
9. Fail-open: `visited_traits.pop()` at 5936 means visited is path-scoped (a trait reachable by two paths is visited twice, producing duplicate edges). That is duplicate not permissive. none seen.

---

## 14. `call_results` (5941-6016)

1. Purpose: type-driven result roster for normal-return obligations at a completed call expression (header 5943-5948: "The roster is provisional: checked bodies owe it at exits, sealed operators owe their admitted catalog contract, and routed rows still require the independent invocation/custody admission in content checking.").
2. Shape: `type_domain_rows` (5963, `pub(super)`), `append_memberships` (5988, `pub(super)`).
3. Decides: `type_domain_rows`: if the return type is a readable reference shell, uses its referee (5967-5969); then `declared_result_field_domain_contracts` + `declared_domain_constraint_records` (5970-5979). `append_memberships` publishes `FactPayload::DomainMembership` with `FactOrigin::CallEnsures` on `PlaceRoot::Expression(expression)` plus path (5999-6011).
7. Caller counts: `type_domain_rows` 2 outside calls (flow_calls.rs:871, 897); `append_memberships` not grepped.
8. Stated limits: "Routed referent duties are proved independently at the callee's exits, not inferred here." (5961-5962).
9. Fail-open: `append_memberships` publishes memberships "without selecting a case or creating an owned claim" (5984-5985), with `value: ExpressionHandle::invalid()` and `domain: HandleSpan::empty()` (6006-6007): each row asserts membership by (domain symbol, semantic domain) only. Whether the callee actually proves it is checked elsewhere (stated 5946-5948). "stated".

---

## 15. `borrow_lifetimes` (6025-6313)

1. Purpose: drops borrow-loan constraints from the active constraint set when the loan has expired (past its last use) or was reassigned (the place that carried it was overwritten), recording a `FlowBorrowWeakeningFact` each time (header 6031-6034).
2. Shape: `filter_expired_borrow_loans` (6046, `pub(crate)`), `weaken_expired_loan` (6084, `pub(super)`), `filter_reassigned_borrow_loans` (6120, `pub(crate)`).
4. Decides:
   - Expiry: `execution_order.position(last_use_statement_index) < current position` (6093-6098). Uses `.expect("loan expiry names an admitted execution point or state exit")` (6095) and `.expect("an expired loan has a following execution boundary")` (6104): panics if not admitted.
   - Reassignment (6120-6312), in two stages: (a) via `stored_reference_fallthrough_effects` — loans whose `storage_owner` equals the effect destination local and whose owner path is covered, and `!effect.may_select` are weakened; `BorrowIncomingReference` with `ParameterReferent` origin are kept (6155-6160); (b) via the assignment destination: a loan is kept if its `storage_owner != destination.root`, or (writes_referent, authored carrier equals destination and owner path length equals destination segment length), or it was made at this statement, or the destination does not cover its owner path (6276-6285).
   - `place_path_covers_owner(program, destination.segments, owner_path)` decides coverage.
5. Cases by shape: assignment target type: a `Reference` (peeled `Constrained`) vs not; for non-declared-type targets requires an array window shape with valid element type and known length (`window`, 6228-6249) else returns `source` unchanged (nothing weakened).
6. Same job elsewhere: loan-weakening and incoming-reference weakening duplicate the "owner path covered by destination" test at 6165-6169, 6181-6185, 6281-6284, 6302 (4 sites).
7. Caller counts: not grepped separately (`filter_expired_borrow_loans` and `filter_reassigned_borrow_loans` are called from `statements` 4844, 4990, 5211 — read).
8. Stated limits: "`None` is that immutable preparation's failed result, not a request to reconstruct the resolver for each loan-lifetime query." (6116-6118).
9. Fail-open: several early `return source` (nothing released): 6146, 6223, 6247, 6258, 6270. Returning `source` keeps loans = more constraints = fail-closed. In stage (a) `effect.may_select` effects never release a loan (6162, 6177-6178). none seen as permissive. Note the `replaced` test requires `active.statement_index < statement_index` (6176), so loans formed at the same statement are kept.

---

## 16. `domain` — `dependencies` (6314-6879) and start of `invalidation` (6880-7200)

### 16a. `dependencies::expression::relative` (6332-6429) and `collect_dependency_paths_from_expression` (6433-6547), `dedupe_dependency_segments` (6549-6566)
1. Purpose: compute, for a domain definition, which place paths (relative to the domain subject) its predicate reads, so a later write to one of those paths invalidates the domain-membership fact.
2. Shape: `relative_place_segments_from_expression` (6338; `pub(crate)`), `resolve_member_symbol_from_type` (6405), `collect_dependency_paths_from_expression` (6433), `dedupe_dependency_segments` (6549).
4. Decides: member symbol resolution is **by name string** against the receiver's type: `validation::data_member_symbol_by_name(program, data, member_name)` / machine owned data `owned.name.as_str() == member_name` (6405-6428), falling back to `effective_member_symbol` (6383). Name subject must satisfy `domain_subject_reference` (6358).
5. Cases by shape: `collect_dependency_paths_from_expression` has an explicit match over every expression node: `SavedSubject, Match, Atomic, ArrayLiteral, Binary, Range, Call, Cast, Indexed, Member, Borrow, Unary, Name, StructLiteral`, and `TypeExpression | spelled_literal_expression_pattern!() => {}` (6544-6545). Exhaustive (no wildcard).
6. Same job elsewhere: `qualification_is_content_independent`'s `mathematical_captures_are_inline` (6648) repeats "relative segments -> reject Index/OpaqueRange -> every prefix has no reference shell" twice (6655-6681 and 6708-6733).
7. Caller counts: `relative_place_segments_from_expression` is re-exported (`pub(crate)` 6431, 6569); not grepped.
9. Fail-open: an expression whose place can't be resolved (neither canonical nor relative) contributes **no** dependency (`Name` arm 6530-6538; for `Member`/`Indexed` it recurses into children 6509, 6521). A domain predicate reading storage through an unrecognised path would depend on nothing, but `matching_dependency_segments` handles an *empty dependency list* as "fall back to fact-place overlap" (6988-6995), so the fact is still invalidated by writes to its own place. Writes elsewhere that the predicate reads but could not be resolved would not invalidate it. "traced" (6988-6995): the fallback covers the fact's own place only.

### 16b. `qualification_is_content_independent` (6590-6646) and `mathematical_captures_are_inline` (6648-6734)
1. Purpose: decides whether a domain's qualification depends only on a descriptor's inline values (not on separately backed content), so a content write need not invalidate it. Header 6583-6589.
2. Shape: `pub(in crate::flow) fn qualification_is_content_independent` (6590), recursion `visit` with `visiting`/`completed` lists (6594).
4. Decides (all keyed on the domain definition, not names): `false` if the domain has a `classification`, any `semantic_roles`, any `operators`, or a present predicate with no proof facts (6609-6616); alias constituents must all be content-independent (6618-6623); membership facts must have the domain subject as value and a content-independent target (6625-6631); expression facts must pass `mathematical_captures_are_inline`; `Proposition` => `false` (6635); cycle (`visiting.contains`) => `false` (6603-6605).
5. Cases by shape: `mathematical_captures_are_inline` accepts: relative place whose every prefix has no reference shell and whose final type is primitive and no `Index`/`OpaqueRange` segments (6655-6681); literal Boolean/Integer/Float (6683-6687); Binary/Unary recursively (6688-6694); otherwise prepared-mathematics application captures each of which is a relative place without index/range, with non-reference prefixes and primitive `source_type` (6697-6733).
7. Caller counts (grep): `qualification_is_content_independent` 3 outside uses (flow_mutation.rs:2373 via import 2248, flow_expression.rs:5404, flow_call_phases.rs:1287).
9. Fail-open: absent domain definition => false (6606-6608); `prepared_mathematics()` None => false (6697-6699). A domain with *no* predicate and no aliases/memberships: `domain.predicate_presence.is_present()` false skips the predicate guard, `aliases` true (`alias.as_ref().is_none_or`), `facts` true over empty `proof_facts` => returns **true** (content-independent) (6609-6641). Wrong program that gets through: a domain whose predicate is absent but which is not truly "inline-only" — e.g. an opaque domain with no proof facts and no predicate presence — is deemed content independent, so content writes would not invalidate qualification. "open": depends on whether `predicate_presence` is the only way a domain can have hidden content dependence; `classification.is_some()` and `operators` are excluded.

### 16c. `build_domain_facts` (6736-6769), `domain_dependency_segments` (6771-6802), `compute_domain_dependency_segments` (6804-6878)
1. Purpose: build `DomainFacts` (per-domain dependency paths) before flow.
2. Shape: `DomainDependencyCache { by_domain }` (6573) / `DomainDependencyCacheEntry` (6578); `build_domain_facts` (6736, `pub(crate)`).
3. Inputs/outputs: reads `program.domain_definitions()`, `semantic.facts_for_symbol(domain_symbol)`. Output `DomainFacts::with_roots(segments, dependency_paths, dependencies)` (6768).
4. Decides: for each fact of the domain symbol: `BooleanExpression` => collect paths from the expression; `DomainMembership` of an imported domain => rebase the imported domain's dependency list under this place (6838-6869); no place => push empty path = depend on everything (6842-6845). Cycle (`visiting.insert` false) => returns `vec![Vec::new()]` i.e. "depends on the whole value" (6819-6821); missing definition => `vec![Vec::new()]` (6824-6827).
6. Same job elsewhere: the cache is a `Vec` scanned linearly (6777-6780, 6798-6800, 6811-6815) — "cache" tracked in `tools/cache_state.txt` per AGENTS.md (not checked).
8. Stated limits: none; "An empty ordinary dependency roster is not this proof: it can mean unknown." (6588-6589).
9. Fail-open: empty path `Vec::new()` means whole value, conservative (more invalidation). none seen.

### 16d. `invalidation::matching` (6892-7008) and the mutation filters (7010-7200)
1. Purpose: decide whether a write overlaps a fact (directly or through a domain's dependency paths), and drop contexts/facts accordingly, recording `FlowInvalidationFact` rows.
2. Shape: `matching_mutation_for_fact_place` (6901, `pub(in crate::flow)`), `matching_dependency_segments` (6972), `context_survives_place_mutations` (7016, `pub(crate)`), `visit_context_invalidations` (7043), `filter_contexts_after_place_mutations` (7075, `pub(crate)`), `filter_contexts_after_mutations` (7113, `pub(in crate::flow)`), `ContextSurvival {Unchanged, Dropped, Reduced}` (7173), `context_survival` (7184 start; body continues beyond 7200).
4. Decides:
   - `matching_mutation_for_fact_place`: no mutated places => None; fact root `Unknown`, invalid symbol, or invalid expression => **None (not invalidated)** (6913-6920); only mutations with the same root are considered (6928-6931, 6945); for `CaseValue` facts only mutations that `case_overwrite` count (6948-6957); for domain-membership facts the domain's dependency paths are used (6937-6942, 6997-7006), otherwise fact-place overlap `canonical_place_segments_may_overlap`.
   - `visit_context_invalidations` (7043-7073): a fact with no place is "invalidated" only if payload is `StorageDependency`; otherwise skipped (7061-7066).
5. Cases by shape: root match list (6913-6926): `Unknown` and invalid `Symbol`/`Expression` roots are never invalidated; `Symbol | InvocationReferent | InvocationOutput | Expression | TypeReference` proceed.
6. Same job elsewhere: `context_survives_place_mutations` (7016) and `filter_contexts_after_place_mutations` (7075) both wrap `matching_mutation_for_fact_place` — a query form and a publishing form (stated 7012-7014: "Query the same storage/dependent-domain invalidation law without publishing a second flow trace").
7. Caller counts: `context_survives_place_mutations` 1 outside caller: published_facts.rs:1894; re-exported at flow.rs:7287 and 11733.
8. Stated limits: "Facts that overlap the write leave the context. A storage dependency and the fact it guards leave together. Every other fact in that context stays: independent field domains are not one proposition." (7179-7183).
9. Fail-open:
   - `matching_mutation_for_fact_place` `PlaceRoot::Unknown`/invalid root => `None` (6914-6920): a fact over an unknown or invalid root survives every write. Reached by any fact whose place root is unknown (e.g., `FactPlace::Place` with `PlaceRoot::Unknown`); facts with `FactPlace::Unknown` skip via 7061-7065. Wrong program that passes: a fact about an unresolved place is never retired by a write. "open" (no comment gives a reason; not traced to who creates such roots). The operator-ensures `FactPlace::Unknown` fact (4434) is one producer ("traced": read 4432-4449, 7061-7066).
   - `matching_mutation_for_fact_place` only compares mutations with identical `root` (6945): a write to a *different* root that aliases the fact's root is not seen here; aliasing is handled by callers via `extend_invocation_observations` / `close_storage_places_over_origins` (5415-5421, 5525). "stated" via 2047-2050 and 5719-5726.
   - `matching_dependency_segments`: no dependency row (domain unknown to `DomainFacts`) falls back to fact place overlap (6979-6986). fail-closed-ish.

---

## Cross-range observations

- Same job, repeated in range: (1) construct `FlowOperandReferent` from `CanonicalPlace` (1433-1440, 1471-1488, 1508-1521); (2) "filter contexts after mutations → project constraints → extend invocation observations" (4175-4206, 5524-5549); (3) "relative segments with no Index/OpaqueRange and no reference prefix" (6655-6681, 6708-6733); (4) the prelude of `project_invocation_place` and `nonresident_content_region` (1914-1934, 1994-2014).
- Panics instead of errors in range: `expect` at 5769, 6095, 6104; `assert_eq!` at 5773.
- Spelling-keyed judgments in range: operator resolution by receiver path members and `call.target.as_str()` (3932-3953), relative-place root by `parameter.name.as_str() == head` (4567), member resolution by name (6405-6428), boundary signature by `signature.name == target_state.name` (5908), operand labels via `display_name` (4054, 4103).

---

## Coverage
- Read in full: `flow.rs` lines 1201-7200 = 6000 lines (read in four chunks: 1201-2700, 2701-4200, 4201-5700, 5701-7200).
- Skimmed: none.
- Not read: none of my unit. (Lines 1170-1200 were read as context and do not count.) Body of `context_survival` continues past 7200 and was not read here.

Greps run (Grep tool, across `omega-rust`): `resolve_operator_statement_call|resolve_operator_for_call` (content); `qualification_is_content_independent|context_survives_place_mutations|activation_parameter_origins|address_before_statement|append_preserved_binding|type_domain_rows|named_operator_call_mutated_places` (content); `\.negate\(|\.possible\(|\.covers\(|\.choices\(|SelectionConditions::new|\.never\(\)` (content); `\bcovers\(|machine_parameter_origins|machine_storage_origins|state_parameter_origins|entry_storage_origins|state_origins` (count: 60 in 14 files); `stored_views::...|stored_values::...` (content); `transition_guard_observations|saved_guard_observations|guard_observations\(|expression_is_stable_predicate|reads_survive` (count: 40 in 5 files); `append_call_boundary_edges|invocation_referent\(|project_invocation_place|extend_invocation_observations|nonresident_content_region|rebase_contexts` (count: 30 in 7 files); `\bbefore_snapshot\(|captured_before_snapshot\(|append_writes\(|\bjoin\(&mut` in `src/` (content).

## Five things a maintainer most needs to know about this range
1. `entry_origins` (flow.rs:1798-2860) is the one origin solver: three wrappers (`machine_parameter_origins` 2346, `activation_parameter_origins` 2358, `machine_storage_origins` 2370) call one fixed point (2378) differing only in the destination-index function and whether call frames are supplied. `activation_parameter_origins` has 4 users in checks_contracts_exits.rs. Unknown origin = invalid symbol; empty = unreached (2170-2172).
2. Unknown writes are handled by forgetting: in `statements`, a `None` write footprint empties all active contexts (5462-5470); `invalidates_destination` treats unknown writes as invalidating (1547). By contrast, `matching_mutation_for_fact_place` never invalidates a fact whose place root is `Unknown` or invalid (6913-6920), and facts with `FactPlace::Unknown` are invalidated only if they are `StorageDependency` (7061-7066).
3. Operator `ensures` publication has two parallel paths (statement 4306 and expression-form 4133) that share `append_operator_ensures_context` (4349) but differ on unknown mutable-operand places: statement form returns `None`/unknown (4292-4296) while the expression-form effect function silently omits the operand (4166-4174).
4. `SelectionConditions` (3567) represents "false" as an invalid handle (`never()`, 3592) while also having `Condition::False` (3558); `negate` indexes the arena without an `is_valid` guard (3757), and `possible`/`covers` return false for unresolvable selections (3687-3689, 3729-3731). Callers in checks_contracts.rs, checks_content.rs and checks_multiplicity.rs depend on these (many sites per grep).
5. Several judgments in this range are spelling-keyed rather than identity-keyed: operator resolution (3932-3953), contract relative-place root (4567), member lookup for domain dependencies (6405-6428), boundary signatures (5908).

## What I am unsure about
- Whether `Default` for `ServiceReachSummary`/`SuspensionSummary`/`BlockingSummary`/acknowledgement (used at 1273-1353) means "none" or something permissive; not read.
- Whether `Arena::get` on an invalid handle (3757, `negate(never())`) panics or returns a default; not read.
- Whether downstream callers treat `Some(Vec::new())` from `stored_views` (1647-1678) as "no storage" or "unknown".
- Per-name caller counts for several grouped greps (entry_origins exports, guard predicates, boundary edges) are combined counts.
- Body of `context_survival` (7184-...) continues past my range; its behavior is not recorded here.
- Whether the unrebased-root fact kept by `rebase_contexts` when `target` is invalid and `assumptions` is false (2759, 2775) is later filtered by the `FlowExitParameterOrigin` roster.
