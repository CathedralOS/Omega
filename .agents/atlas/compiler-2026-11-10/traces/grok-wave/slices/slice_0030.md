slice_0030

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0224 | projections.rs:1835-1866 | CLOSED | Unresolved call sites are first-class task-stack metadata, not a compile bypass. |
| A0225 | proofs.rs:2625-2626 | REACHABLE | Policy-path divisor safety treats literal −1 as nonzero; exact-path rejects −1 at proofs.rs:2573. |
| A0226 | proofs.rs:298-308,368-370 | UNDECIDED | Stage 05 entry-claim publication vs lowering drop when qualifications do not map to content was not fully traced. |
| A0227 | proofs.rs:4452-4463 | CLOSED | Stage 05 publication requires nonempty buckets; normalize only strips `Constant(false)` before `CrashRouteBucket::new`. |
| A0228 | proofs.rs:4220,4326 | UNDECIDED | Unhandled `ScalarTerm`/`Proposition` variants left unsubstituted; crash-route term inventory not enumerated. |
| A0229 | proofs.rs:4751-4766,5050,6543 | CLOSED | Omitted undeclared applications error if evidence still references them at 5050-5051 and 6543-6545. |
| A0230 | proofs.rs:7186-7256 | REACHABLE | Checked direct-source projections lower via transitional input when direct binding is absent. |
| A0231 | proofs.rs:7319-7324,7400-7402,7417,7449 | CLOSED | `Ok(None)` resolution paths intentionally feed transitional lowering in entry A0230. |

### A0224

1. Read `task_frame_summary` in full (`projections.rs:1721-1887`, read).
2. Search `rg 'task_frame_summary' omega-rust/psi` → 2 hits (`projections.rs:1678`, `:1721`); caller is `task_call_graph` (`projections.rs:1651-1695`, read).
3. Search `rg 'task_call_graph' omega-rust/psi` → 2 hits (`projections.rs:1651`, `:2306`); caller `lower_task_activation_demands` (`projections.rs:2276-2306`, read through `:2416`); caller `machine_lowering.rs:1795`, read).
4. Input on accepting branch: flow call rows in `task_frame_summary` where `checked_call_target` (`projections.rs:1926-1945`, read) returns `None` (`UnresolvedTarget`, `1837-1850`) or callee `supply_mode != CheckedBody` (`NonCheckedSupply`, `1852-1865`).
5. Entry text matches code at `1835-1866` (read); cited line numbers align.
6. Verdict **CLOSED**: `stack_composition.rs:287` documents nonempty `unresolved_calls` as covering only the resolved subgraph; `foundation_validation.rs:9387-9398` validates unresolved coordinates without requiring resolution. `lower_task_activation_demands` still errors on suspension/crossing violations (`1830-1833`) and invalid targets (`1697-1716`). NonCheckedSupply frames are intentional partial-graph inputs, not ill-formed programs incorrectly accepted.
7. Unsure: whether checked flow can record `UnresolvedTarget` targets that stage 05 should have rejected (did not read stage 05 flow publication for task WCSU calls).

### A0225

1. Read `safe_policy_structural_divisor` in full (`proofs.rs:2613-2645`, read) and caller block (`proofs.rs:7976-7988`, read).
2. Search `rg 'safe_policy_structural_divisor' /agent/repos/omega` → 3 hits (definition and use in `proofs.rs`).
3. Caller `substitute_structural_crash_route_roots` not involved; caller is structural crash arithmetic lowering inside `proofs.rs` (~7940+, read through 7988).
4. Input: signed integer literal divisor with value −1 in policy divide/remainder crash lowering when `formation_decided` is false.
5. Entry accurate at `2625-2626` (read): signed literal branch accepts any nonzero value including −1; contrast `safe_exact_structural_divisor` at `2573` (`*value != -1`, read).
6. Verdict **REACHABLE**: wrapping/saturating structural crash divide with literal −1 divisor passes `safe_policy_structural_divisor` and bypasses the nonzero-divisor evidence check at `7983-7987` despite exact-path rejecting −1 at `2573`.
7. Unsure: whether policy integer divide with −1 is semantically defined as safe in the language spec (did not read wiki).

### A0226

1. Read `lower_whole_content_entry_claims` in full (`proofs.rs:237-392`, read).
2. Search `rg 'lower_whole_content_entry_claims' /agent/repos/omega` → 3 hits (`proofs.rs:237`, `attached_unit.rs:10560`, `unit_composed_control.rs:10401`); read call sites at `attached_unit.rs:10560-10568` only.
3. `continue` when `content.is_empty()` (`306-308`, read) after `filter_map` drops unresolved domains (`298-305`, read); `continue` when `projections.is_empty()` (`368-370`, read).
4. Input: `CheckedUnitEntryClaimPlan` rows whose qualification domains do not resolve via `for_semantic_domain` or yield no projections after mapping.
5. Entry matches code (read); `368-370` is secondary empty-projection drop after errors in the map loop.
6. Verdict **UNDECIDED**: did not trace stage 05 publication of `entry_claims` / `projected_qualifications` to prove whether a checked program can retain a plan row that lowering silently drops without failing compilation.
7. Unsure: stage 05 guards in `execution_terminal_unit_state_graph.rs` around `projected_qualifications` (grep hit `10012` not read).

### A0227

1. Read `normalize_formal_crash_routes` (`proofs.rs:4440-4465`, read) and `CrashRouteBucket::new` (`contract_and_obligation_facts.rs:1400-1415`, read).
2. Callers `lower_boundary_crash_routes` / `lower_formal_crash_routes` at `4402-4420`, read.
3. Input: published crash buckets whose guards normalize to empty after stripping `CheckedBooleanExpression::Constant(false)` (`4452`, read).
4. `filter_map` + `new` dropping empty guard sets (`4462-4463`, read) matches entry; read `new` at `1411-1414`: returns `None` when `alternative_guards.is_empty()`.
5. Verdict **CLOSED**: stage 05 `published_facts.rs:4360-4364` (read) requires `CrashRouteBucket::new` succeed when building publication; lowering normalization only removes constant-false guards and cannot empty a bucket that still has non-false guards or `Truth`.
6. Unsure: none for publication-vs-lowering alignment on this path.

### A0228

1. Read `substitute_structural_crash_route_roots` with nested `substitute_term` and `substitute_proposition` (`proofs.rs:4163-4348`, read).
2. Search `rg 'substitute_structural_crash_route_roots' /agent/repos/omega` → hits in `unit.rs`, `proofs.rs`, `attached_unit.rs:13337` (call site not read in full).
3. Input: crash-route predicates containing `ScalarTerm` variants not matched in `substitute_term` (`4220` `_ => {}`, read) or `Proposition` variants handled only with no-op at `4326` (read).
4. Entry matches code (read).
5. Verdict **UNDECIDED**: did not enumerate which `ScalarTerm`/`Proposition` forms appear in lowered crash routes after `lower_structural_crash_route_buckets` to judge whether unsubstituted nodes still carry stale `PlaceId`s.
6. Unsure: attached_unit substitution closure at `13337` (not read in full).

### A0229

1. Read `lower_proposition_vocabulary` loop (`proofs.rs:4743-4793`, read) and consumers at `5050-5051` and `6543-6545` (read).
2. Search `rg 'lower_proposition_vocabulary' /agent/repos/omega` → 3 hits; production caller `lower_and_install_evidence_artifacts` at `6691` (read `6689-6698`).
3. Input: proposition applications whose `declaration` symbol is absent from `declaration_ids` (`4751-4755` `continue`, read).
4. Entry matches omit behavior (read); lookups at `5050` and `6543` error when match fails (read).
5. Verdict **CLOSED**: silent omit affects only applications not needed downstream; retained evidence terms and proof-output paths fail closed with `LoweringError::Unsupported` if a needed application was omitted.
6. Unsure: whether checked facts can contain orphan applications with no declaration that nothing references (did not read stage 05 proposition publication).

### A0230

1. Read `lower_float_meaning_projection` in full (`proofs.rs:7163-7292`, read).
2. Search `rg 'lower_float_meaning_projection' /agent/repos/omega` → 2 hits; caller `retain_float_meanings` `machine_lowering.rs:1106-1118` (read).
3. Input: `CheckedFloatProjectionSource::Direct*` variants with `direct_source == None` at lowering (`7177-7186`, `7191-7200`, etc., read) — each falls through to `transitional_input(...)`.
4. Entry matches transitional fallback (read).
5. Verdict **REACHABLE**: a checked projection authored with a direct source still lowers when binding resolution fails, emitting `FloatMeaningSource::TransitionalInput` instead of failing lowering.
6. Unsure: runtime obligations on transitional sources (`terminal-psi/semantic_module.rs:566` not read).

### A0231

1. Read `resolve_direct_float_source_binding` in full (`proofs.rs:7294-7451`, read) including callees `resolve_direct_float_parameter` (`7453-7507`, partial read) and block resolver header (`7510-7540`, read).
2. Search `rg 'resolve_direct_float_source_binding' /agent/repos/omega` → caller `machine_lowering.rs:1107-1116` (read).
3. Input reaching `Ok(None)`: owner machine absent from `machine_bindings` (`7319-7323`); no matching call occurrence (`7400-7401`); `DirectOperationResult` arm (`7417`); default `_` (`7449`); parameter type mismatch returns `Ok(None)` at `7486` (read).
4. Entry cites match cited lines (read); feeds A0230 via `direct_source` optional at `1107-1117`.
5. Verdict **CLOSED**: `Ok(None)` is deliberate deferral to transitional lowering in A0230, not an alternate acceptance path; wrong-kind `Some(direct_source)` still errors at `7183-7184` / `7197-7198` (read).
6. Unsure: whether every checked direct projection must appear in `projection_sources` passed from `machine_lowering.rs:2049` (not read).

### What I did not read

- Stage 05 flow publication for task WCSU call targets beyond `shared.rs:338` header.
- Full `execution_terminal_unit_state_graph.rs` entry-claim qualification guards.
- Full `attached_unit.rs:13337` crash substitution caller.
- Complete inventory of `ScalarTerm` variants in lowered crash predicates.
- Stage 05 proposition vocabulary publication.
- `validate_runtime_evidence_inputs` and downstream transitional float semantics.
- `resolve_direct_block_float_parameter` body beyond `7540`.
- Corpus / digest records beyond cited code.

SLICE COMPLETE
