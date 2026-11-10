slice_0054

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0409 | contract_entailment.rs:73357-73359 | CLOSED | No-graph early `true` is bypassed on checked-body paths that attach a flow graph and gate validity on `graph_storage_mode`. |
| A0410 | contract_entailment.rs:76996-76997 | CLOSED | Early `true` only when `!requires_value_preservation()`; runtime entailment judges keep preservation on. |
| A0411 | contract_entailment.rs:83286-83287 | CLOSED | Vacuous `Proven` for negation requires `hypotheses_contradictory` already set by judge intake, not missing input. |
| A0412 | contract_entailment.rs:85364-85368 | UNDECIDED | Function skips `integer_premises_contradictory`, but callers usually pre-filter; cached transition path may not re-check. |
| A0413 | contract_entailment.rs:88447-88449 | REACHABLE | Declared types without `reference_shell` treat terms (including opaque placeholders) as observed before the denying `match`. |
| A0414 | contract_entailment.rs:89539,89638,89738 | REACHABLE | `StructuralTermMatcher` equates `Opaque` terms solely by equal `display_name` strings. |
| A0415 | contract_entailment.rs:90209-90216 | CLOSED | Vacuous `true` is tied to `hypotheses_contradictory` on the same requires-built judge, not an unconditional prove-anything skip. |
| A0416 | contract_entailment.rs:90730-90736;91002-91007 | REACHABLE | Duplicate ensures/requires auto-proven when calls are “declared” by spelling even if resolution is name-only. |

### A0409

1. Read `current_value_valid` in full at `omega-rust/psi/compilation/validation/src/contract_entailment.rs:73351-73532` (read).
2. Search `\bcurrent_value_valid\b` in `omega-rust/`: **10 hits**, all in `contract_entailment.rs` (read).
3. Callers read: `rejoin_followed_path` `73209` (returns early when `storage_graph()` is `None` at `73191-73192`, read); `storage_value_transfer_is_current` `75288-75296`; `current_storage_value_at` `75820-75828`; scalar read fallback `74586-74593`; `observe_captured_store` `76750-76752`. Did not read full bodies of `72013` / `75169` call chains beyond those lines.
4. Upward: `validate_machine_contract_entailment` `90324` builds runtime judges via `from_runtime_state_requires` `90387` (read); checked programs get `storage_graph` at `published_facts.rs:11131` (read).
5. Input on accepting branch: `self.storage_graph()` is `None` inside `StructuralJudge::storage_graph` delegating to `program.storage_graph()` `72974-72975` (read).
6. **Verdict CLOSED:** At `75826-75828` (read), when `graph_storage_mode()` is false, validity does not depend on `current_value_valid`; when graph mode is true, production checking supplies a graph (`published_facts.rs:11131`). The `73357-73358` branch is the intentional no-history fallback aligned with `checking.rs:468-469` (read), not the main checked-body acceptance path.
7. Unsure: Whether any entailment path still runs with `storage_version` set and graph missing outside tests (`72921-72929` test fixture, read).

### A0410

1. Read `entry_storage_observations_preserved` at `76992-77008` and `storage_observation_invalidated` `77010-77023` (read).
2. Search `\bentry_storage_observations_preserved\b` in `omega-rust/`: **3 hits**, all `contract_entailment.rs` (read).
3. Callers: `judge_proposition` `83312-83313` (read); second use `83394` (not fully read).
4. Upward: `validate_machine_contract_entailment` uses `from_runtime_state_requires` with `runtime_body_values = true` `81036` (read); `requires_value_preservation` `81087-81088` (read).
5. Input on accepting branch: judges with `!requires_value_preservation()` (proof-only `runtime_body_values: false` at `81176`, or `case_facts_only` after `restrict_to_case_facts` `81098-81099`).
6. **Verdict CLOSED:** Runtime entailment keeps `requires_value_preservation()` true, so stale observations are checked via `storage_observation_invalidated` / `graph_observation_invalidated`.
7. Unsure: None for main `validate_machine_contract_entailment` path.

### A0411

1. Read `judge_proposition` at `83272-83349`, focusing on unary negation `83280-83290` (read).
2. Search `hypotheses_contradictory` in `contract_entailment.rs`: many hits; assignment sites read at `51417`, `51993`, `52078`, `52105` (read).
3. Callers of `judge_proposition`: `90702`, `90581`, `90587`, `47008`, `77580-77581`, etc. (signatures/lines read; not every caller body).
4. Upward: `validate_machine_contract_entailment` filter at `90730+` and structural judging `90565+` (read).
5. Input: unary `LogicalNot` when `self.hypotheses_contradictory` is already true (read `83286-83287`).
6. **Verdict CLOSED:** Branch is deliberate ex falso; flag is set only after intake detects contradiction (e.g. refuted case `51417`, mismatched constructors `51992-51993`, unequal integers `52077-52078`, read).
7. Unsure: Whether every contradictory requires surface is rejected before machine acceptance (no `hypotheses_contradictory` diagnostic search in `program_validation.rs`, read grep).

### A0412

1. Read `visit_non_contradictory_arm` `85350-85374` (read); contrast `arm_premises` `85647-85659` and `AdmittedArm::premises` `85549-85554` (read).
2. Search `\bvisit_non_contradictory_arm\b` in `omega-rust/`: **4 hits** (read).
3. Callers read: prefix walk `44841-44880` (integer + hypotheses check before call); transition walk `45244-45275` (read).
4. Input: arms whose `hypotheses_contradictory` is false but `integer_premises_contradictory()` might be true (`85364-85373` does not test integer contradiction, read).
5. **Verdict UNDECIDED:** Entry description matches code at `85365-85366` (only `hypotheses_contradictory`). Most call sites pre-filter at `44843-44845` and `45260-45261` (read), but `45248-45258` re-enters without re-checking integer contradiction when `checked_contradiction_state` matches `(integer_premise_revision, predicates.len())` (read). I did not prove whether integer contradiction can change without updating that pair (`integer_premises_contradictory` `53351-53362`, read).
6. Unsure: Invalidation of `cached_prepared_integer_premises` vs `integer_premise_revision` bumps (`79063+`, not fully traced).

### A0413

1. Read `scalar_operand_is_observed` `88424-88512` in full (read).
2. Search `\bscalar_operand_is_observed\b` in `omega-rust/`: **64 hits** in `contract_entailment.rs` (count read; individual caller bodies mostly not read).
3. Upstream example: `current_storage_value_at_entry` `75802-75808` refuses when `scalar_operand_is_observed` is false (read).
4. Input: `declared` with `reference_shell(program, declared).is_none()` and a term such as `Opaque`, `Identity`, `LogicalNot`, or `MathematicalBinary` that reaches `88447-88448` before the `match` arms that return false at `88504-88510` (read).
5. **Verdict REACHABLE:** A checked-body machine whose storage model treats an unresolved opaque placeholder as an observed primitive operand could continue proof/storage capture that should require a real observation.
6. Unsure: How often shell-none declared types coincide with those term shapes in production witnesses (not traced).

### A0414

1. Read `structural_term` opaque fallbacks `89539-89541`, `89638-89640`, `89738-89740` (read); matcher `79934-79938` (read).
2. Note `diagnostic_shape_match` treats `(Opaque, Opaque)` as false at `18966` (read) — different job from matcher.
3. Input: distinct unsupported member/call expressions folded to `Opaque(display_name)` with identical display text in one frame (read).
4. **Verdict REACHABLE:** Matcher equality on display strings can identify distinct expressions for rewrite/intake (`52067-52075` uses rewrites, read header only).
5. Unsure: Frequency of display-name collisions for distinct handles (not searched).

### A0415

1. Read `premises_prove` `90180-90217` and caller `transparent_application_entailed` `90090-90143` (read).
2. Search `\bpremises_prove\b` in `omega-rust/`: **3 hits** in `contract_entailment.rs` (read).
3. Input: `conjuncts` list with judge where `judge.hypotheses_contradictory` OR each `judge_observed_predicate` is `Proven` (`90209-90215`, read).
4. **Verdict CLOSED:** Same vacuous mechanism as A0411, scoped to transparent requirement entailment; not an unconditional accept when hypotheses are consistent.
5. Unsure: Same as A0411 regarding whether contradictory requires always fail earlier.

### A0416

1. Read ensures filter `90730-90736` and `fact_call_targets` call case `91001-91018` (read).
2. Upward: `validate_machine_contract_entailment` pushes matching ensures to `proven.entry` without further structural proof (read `90734-90735`).
3. Input: ensures fact structurally equal to some requires (`expressions_structurally_equal`) and call targets all marked declared because `call.target.as_str()` matches some machine name at `91006-91007` even when overload resolution is absent (read).
4. **Verdict REACHABLE:** A machine could get an ensures clause proven as a tautological copy of requires plus a call considered “declared” by name spelling alone, without proving the call’s semantics.
5. Unsure: Whether `expressions_structurally_equal` already forces enough resolution that the name-only declared bit is redundant (not read).

### What I did not read

- Full bodies of all `scalar_operand_is_observed` call sites (64 hits).
- Complete `recognize_structural_state_prefix` / transition walker beyond cited contradiction and `visit_non_contradictory_arm` call sites.
- `rewrite_table` integer-premise cache invalidation logic beyond line citations.
- `expressions_structurally_equal` implementation.
- Test-only modules around `72921-72949`.

SLICE COMPLETE
