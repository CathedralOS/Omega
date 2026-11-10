slice_0057

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0433 | domains_and_predicates.rs:7976-7987 | CLOSED | Early return runs only when no domain fact names the written field and the field type has no establishment obligation; program acceptance still requires `validate_default_domain_writes` diagnostics. |
| A0434 | domains_and_predicates.rs:8005-8049 | CLOSED | Filtering drops requirements judged disjoint via `fact_is_disjoint_from_store`; that is intentional narrowing, not silent acceptance. |
| A0435 | domains_and_predicates.rs:8982-9019 | CLOSED | Each return is a deliberate guard; data-type reads and place reads are covered by sibling checks in `scan_expression_use` / `validate_data_read`. |
| A0436 | domains_and_predicates.rs:8966 | CLOSED | `_ => {}` covers only leaf `ExpressionNode` variants without child operands; the entry mislabels them as unlisted compound kinds. |
| A0437 | domains_and_predicates.rs:9939-9941 | REACHABLE | Any state parameter symbol is treated as established regardless of whether its type requires default-domain establishment. |
| A0438 | domains_and_predicates.rs:11400 | REACHABLE | Unresolved transition targets omit graph edges, so successor states can skip predecessor window/establishment meet. |
| A0439 | domains_and_predicates.rs:11807-11812 | REACHABLE | After symbol mismatch, case membership falls back to spelling equality, so same-spelled variants compare equal. |
| A0440 | domains_and_predicates.rs:12902-12904;13231-13233 | REACHABLE | A callee can register a successful parameter proof once and suppress caller diagnostics while another call path records nothing. |

### A0433

1. Read `handle_field_store` in full (`domains_and_predicates.rs:7776-8161`).
2. Caller search: `rg '\bhandle_field_store\(' omega-rust` → **2 hits** (definition + call at `domains_and_predicates.rs:7560` inside `handle_assignment`).
3. `handle_assignment` (`domains_and_predicates.rs:7355-7578`) is reached from `walk_state` (`domains_and_predicates.rs:13409`); `walk_state` is driven by `analyze_default_domain_writes_with_frames` (`domains_and_predicates.rs:12595`), which feeds `validate_default_domain_writes` (`domains_and_predicates.rs:12582-12592`) and then `program_validation.rs:4675-4679` inside `judge_contract_entailment`.
4. Input on the branch: a field store whose field name is absent from every requirement’s `fact_names_field` (`domains_and_predicates.rs:7976-7982`) and whose field type does not require establishment (`domains_and_predicates.rs:7983-7985`).
5. **Verdict: CLOSED** — the skipped loop at `8064-8146` is vacuous for obligations naming this field; open-window reads still error in `validate_qualified_place_read` (`9065-9069`) and `validate_data_read` (`9157-9175`). I did not read `fact_is_disjoint_from_store` internals.

### A0434

1. Read the filter closure in full (`domains_and_predicates.rs:8002-8049`) inside `handle_field_store`.
2. Caller: same chain as A0433 (`handle_field_store` → `handle_assignment` → `walk_state` → `validate_default_domain_writes`).
3. Input: an already-established place with a domain requirement on a fact judged disjoint from the store destination path (`8016-8035`).
4. **Verdict: CLOSED** — `disjoint` uses `fact_is_disjoint_from_store` (`8021-8026`); dropped requirements are not treated as discharged elsewhere in this function.

### A0435

1. Read `validate_qualified_place_read` in full (`domains_and_predicates.rs:8970-9071`) and caller `scan_expression_use` (`8667-8968`).
2. Caller search: `rg '\bscan_expression_use\b' omega-rust` → **4 hits** (all `domains_and_predicates.rs`); `validate_qualified_place_read` has **2 hits** (definition + call at `8714`).
3. Upward: `scan_expression_reads` (`8642-8654`) → statement scan (`8621-8632`) in `walk_state`; acceptance via `analyze_default_domain_writes_with_frames` → `judge_contract_entailment` (`program_validation.rs:4675`).
4. Inputs per return arm (read at cited lines): non-place expression (`8982-8989`); no declared place type (`8991-8997`); type without establishment (`8999-9000`); non-consuming non-constrained read (`9002-9008`); nominal data type (`9010-9015`); missing place spelling (`9017-9018`).
5. **Verdict: CLOSED** — `9010-9015` defers to `validate_data_read` / `validate_read!` on `Name`/`Member` paths (`8788-8860`, `9147-9175`). Remaining arms exclude cases this pass is meant to police. I did not read full `declared_place_type_raw` (`places.rs:1275`).

### A0436

1. Read `scan_expression_use` match in full (`domains_and_predicates.rs:8787-8967`).
2. Cross-read `ExpressionNode` enum (`typed-trees/src/values.rs:4199-4221`).
3. **Entry description wrong (read):** `_ => {}` covers leaf variants (`Boolean`, `Integer`, `String`, `Float`, `TypeExpression`, `ZeroValue`, `SavedSubject`, `UndeterminedNaN`) with no child expressions to scan—not omitted compounds (`Match`, `Call`, etc. are listed).
4. **Verdict: CLOSED** — no accepting branch; compound kinds recurse via `read_child!`.

### A0437

1. Read `declared_root_established` in full (`domains_and_predicates.rs:9931-9966`).
2. Caller search: `rg '\bdeclared_root_established\(' omega-rust` → **6 hits** in `domains_and_predicates.rs` (definition + uses at `8199`, `9039`, `9145`).
3. Upward: used in `validate_qualified_place_read` (`9039-9043`) and `validate_data_read` (`9145-9146`) under the default-domain walk → `validate_default_domain_writes`.
4. Input: any valid state-parameter root symbol (`9939-9940`) without regard to parameter type.
5. **Verdict: REACHABLE** — a program could treat a state parameter whose type requires establishment as already established for qualified reads before any in-state constructor runs; such a program could pass default-domain read checks when it should not. I did not read parameter-type admission in `machine_parameters_and_bindings.rs`.

### A0438

1. Read `state_edges` in full (`domains_and_predicates.rs:11373-11408`) and the fixed-point loop (`domains_and_predicates.rs:12754-12877`).
2. Caller search: `rg '\bstate_edges\(' omega-rust` → **3 hits** (`11373`, `12650`, `12754`).
3. Input: a `TransitionTargetNode::Named` path that fails both symbol match (`11389`) and final-member name match (`11390-11398`), so `to` stays `None` and no edge is pushed (`11400-11402`).
4. **Verdict: REACHABLE** — a successor state with no resolved predecessors keeps empty `entry_established` / `entry_windows` (`12811-12812`, `12831-12852`) instead of inheriting an open window from the intended predecessor; reads there may pass when the window should still block. I did not read transition validation in `declarations_core.rs`.

### A0439

1. Read `case_membership_value` (`domains_and_predicates.rs:11797-11813`) and caller `fold` arm (`11652-11666`).
2. Caller search: `rg '\bcase_membership_value\b' omega-rust` → **2 hits** (definition + `11666`).
3. Input: case-membership expression whose `resolved_symbol` is not active, but whose last path-member spelling equals `program.symbols.name(active)` (`11807-11812`).
4. **Verdict: REACHABLE** — two distinct variants that share a display spelling can make membership evaluate true during predicate fold (`11652-11666`), skewing store proof in `handle_field_store`. I did not read variant-name uniqueness validation.

### A0440

1. Read aggregation loop (`domains_and_predicates.rs:12896-12907`) and `restore_parameter_write` (`13180-13269`), including early return (`13231-13233`).
2. Caller search: `rg '\brestore_parameter_write\b' omega-rust` → **5 hits** (definition + four call sites `13476`, `13515`, `13556`, `13671`).
3. Input: callee machine called as parameter—one call with matching `saved_places` records `(symbol, true)` (`13263-13267`); another call leaves `entry_valuations` empty and returns without recording (`13231-13232`); aggregation sees all recorded attempts proved (`12902-12903`) and drops caller `machine_diagnostics`.
4. **Verdict: REACHABLE** — callee can leave a parameter window open on an untracked argument path while a tracked call’s proof causes diagnostics suppression for the whole machine. I did not read full `walk_state` call handling beyond cited call sites.

### What I did not read

- `fact_is_disjoint_from_store` and `predicate_read_set` implementations (`place_queries` module).
- Full `walk_state` (`domains_and_predicates.rs` ~13300+ beyond cited statements).
- `declared_place_type_raw` beyond `places.rs:1275-1319`.
- Variant naming / transition resolution validators outside `domains_and_predicates.rs`.
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checking.rs` beyond grep hits for `judge_contract_entailment`.

SLICE COMPLETE
