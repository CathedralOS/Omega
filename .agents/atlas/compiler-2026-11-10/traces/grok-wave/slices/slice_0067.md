slice_0067

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0513 | places.rs:6124-6128 | REACHABLE | Private `carriers_match` treats invalid-symbol `Named` carriers as equal on display name when deciding cast place preservation. |
| A0514 | places.rs:1618-1626 | REACHABLE | `ExtentResolver::reference` parses a `Named` operand’s spelling as an integer when the symbol handle is invalid. |
| A0515 | program_validation.rs:74-79 | CLOSED | Assembly cyclic/non-finite order is rejected in pipeline 04; other states always get `Some` order so the missing-return pass runs. |
| A0516 | program_validation.rs:4927-4929 | REACHABLE | Cleanup normal-completion walk skips targets missing from `OperationalPlan` with no diagnostic. |
| A0517 | program_validation.rs:4555-4570 | REACHABLE | Sole exact-overflow errors are stripped before `finish_diagnostics`; `validate_program` never merges `deferred_overflow`. |
| A0518 | program_validation.rs:860-887 | REACHABLE | Entailment `reuse` keys on source text hash and dependency context, not full semantic tables. |
| A0519 | proof_contracts_core.rs:217-221 | CLOSED | Quotient formation requires a data carrier symbol before `first_forbidden_quotient_content` runs (quotients.rs:553-562). |
| A0520 | proof_contracts_core.rs:795-801 | REACHABLE | Missing or non-transparent proposition definitions make `proposition_has_source_builtin_index_meaning` return true. |

### A0513

1. Read `carriers_match` in full at `places.rs:6096-6150` (read); entry matches lines 6124-6128 and 6142-6146.
2. Search `\bcarriers_match\(` under `omega-rust/`: **2 hits**, both in `places.rs` (definition and one call at 6083) (read).
3. Read caller `cast_shares_storage` `places.rs:6058-6150` (read); callers `cast_preserves_place` `6033-6056`, `canonical_place_from_expression` `5968-5969`, `place_root_may_be_symbol` `6165` (read, not every downstream custody path).
4. Input: cast operand and target are unwrapped `Named`/`Generic` references whose symbol handles are invalid but display/base names are equal; `carriers_match` returns true and `cast_shares_storage` may return true.
5. Verdict **REACHABLE**: two distinct ill-resolved nominal carriers spelled alike can be treated as sharing storage for place/canonicalization, so a program that should fail overlap or borrow checks could pass validation.
6. Unsure: whether every production typed program still has distinct valid symbols before validation reaches these casts.

### A0514

1. Read `ExtentResolver::reference` in full `places.rs:1602-1638` (read); entry matches 1618-1626.
2. Search `ExtentResolver|resolved_array_extent` under `omega-rust/`: many hits; production callers include `checks_ranges.rs:10748`, `contract_entailment.rs:10494` (read, did not read full caller bodies).
3. Read `resolved_array_extent` wrapper `places.rs:1506-1528` (read).
4. Input: `FixedArrayLength::Expression` reference is `TypeReferenceNode::Named` with `!symbol.is_valid()` and a name that parses as `i128`.
5. Verdict **REACHABLE**: a mistyped or unresolved extent name that is numeric text is accepted as a closed extent instead of failing as an unknown symbol.
6. Unsure: whether the typed-trees stage always rejects invalid-symbol named extents before validation calls `resolved_array_extent`.

### A0515

1. Read `completion_results::validate` in full `program_validation.rs:59-157` (read); entry matches 74-79.
2. Search `completion_results::validate`: **1 hit**, `program_validation.rs:4284` inside `validate_machine` (read).
3. Read `validate_machine` prologue `4275-4284` (read); read `finite_execution_order` `calls_and_control_flow.rs:2382-2388` and graph build `2390-2497` (read).
4. Input: only `Ok(Some(order))` runs the missing-return check; `Ok(None)`, `Ok(None)` from cyclic assembly, or `Err` from `control_graph` returns silently.
5. Verdict **CLOSED**: `pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:2043` runs `assembly_control::admit`, which errors on `finite_execution_order` `Err` (`1360-1365`) or `None` (`1366-1370`); states without assembly get a single `End` block so `finite_execution_order` returns `Some` (`calls_and_control_flow.rs:2492-2497`, `2341-2342`).
6. Unsure: none for the normal pipeline path; did not trace ad-hoc `CheckingInput` test fixtures that skip stage 04.

### A0516

1. Read `validate_cleanup_normal_completion` `program_validation.rs:4906-4970` (read); entry matches 4927-4929 `continue`.
2. Search `validate_cleanup_normal_completion`: **1 caller** `admit_prepared_program_effects` `4899` (read); that caller ends with `finish_diagnostics(diagnostics)` `4903` (read).
3. Read `machine_index_by_symbol` `flow-effects/src/lib.rs:245-250` (read).
4. Input: `CleanupDependency.target_machine` symbol has no index in the passed `OperationalPlan`; the loop skips expansion and never emits the “must return normally” error for that target.
5. Verdict **REACHABLE**: a cleanup dependency edge pointing at a machine absent from the operational plan can pass `admit_prepared_program_effects` when no other diagnostic is raised.
6. Unsure: whether effect planning always registers every `cleanup_dependencies` target (did not read operational-plan construction).

### A0517

1. Read deferral block `program_validation.rs:4554-4570` and `is_exact_overflow_diagnostic` `4645-4648` (read).
2. Read `finish_diagnostics` `4819-4826` (read); read `validate_program` `3946-3967` (read); read pipeline merge `checking.rs:1057-1149` (read).
3. Input: diagnostics contain only errors matching the exact-overflow message pattern; they are moved to `deferred_overflow` and `finish_diagnostics` on the drained list succeeds.
4. Verdict **REACHABLE**: `validate` returns `Ok(ProgramValidation { deferred_overflow, .. })`; `validate_program` checks `discharge_storage_formations` and entailment but never `deferred_overflow` (`3946-3967`); a caller using only `validate_program` or ignoring the field accepts a program whose only errors are deferred overflows. Main pipeline closes at `checking.rs:1148-1149`.
5. Unsure: external API consumers beyond the traced 05 path.

### A0518

1. Read `DependencyBodyIndex::reuse` in full `program_validation.rs:860-887` (read); read `same_body_and_context` `1028-1030`, `local_body_hash` `1216-1275`, `hash_fact` `1304-1326` (read).
2. Search `\.reuse\(` on `body_index` / `DependencyBodyIndex`: caller `judge_machines` `4736` → `judge_contract_entailment` `4668-4687` (read).
3. Input: prior judgment matches machine identity, `body_hash`, `dependency_context`, callee admissions, and ensures count; reuse returns `Some(ProvenContractExpressions)` without re-running entailment.
4. Verdict **REACHABLE**: reuse treats statement and contract-fact source text (and hashed dependency context) as sufficient; semantic changes elsewhere in the typed program that do not change those hashes can still reuse prior proofs.
5. Unsure: full list of semantic inputs omitted from `local_body_hash` beyond what was read.

### A0519

1. Read `first_forbidden_content` `Named` branch `proof_contracts_core.rs:177-235` (read); entry matches 217-221 for `ValueObservation`/`SharedObservation`; for `ContentUse::Quotient` and missing parameter the branch returns `None` (no violation) (read).
2. Search `first_forbidden_quotient_content`: **2 hits** — definition and `quotients.rs:564` (read).
3. Read quotient formation `quotients.rs:545-580` (read): `base_data_symbol` then `data_definition_by_symbol` or error before fence call.
4. Input at cited lines: carrier type name is not a data definition and not a data type parameter; for Quotient purpose no `CarrierFenceViolation` is produced at that site.
5. Verdict **CLOSED** for quotient declarations: unknown/non-data carrier names are rejected at `quotients.rs:553-562` before `first_forbidden_quotient_content` runs on the declared carrier.
6. Unsure: nested field types inside an otherwise valid carrier (walk at `proof_contracts_core.rs:411-418`) were not fully traced against other field validators.

### A0520

1. Read `proposition_has_source_builtin_index_meaning` in full `proof_contracts_core.rs:781-816` (read); entry matches 795-801.
2. Search `proposition_has_source_builtin_index_meaning`: **3 hits** in `proof_contracts_core.rs` (read); `fact_has_source_builtin_index_meaning` caller chain includes `contract_entailment.rs:15597-15600`, `15226-15239` (read, not full `entailed`).
3. Input: proposition symbol has no `proposition_definition_by_symbol`, or definition body is not `Transparent`; function returns `true` after argument checks pass.
4. Verdict **REACHABLE**: contract facts using such propositions pass the “source builtin index meaning” gate (`entailed` does not reject at `15597-15600`), so obligations that should depend on resolving the proposition may be treated as supported.
5. Unsure: whether well-typed programs reaching validation can still omit proposition definitions (did not trace symbol-resolution stage).

## What I did not read

- Full bodies of all `canonical_place_from_expression` and custody callers for A0513.
- Operational-plan construction that fills `cleanup_dependencies` and machine indices (A0516).
- Complete entailment proving after `entailed` passes the index-meaning filter (A0520).
- All call sites of `resolved_array_extent` beyond the two grep hits (A0514).
- `hash_fact` / dependency-context hashing beyond the read ranges (A0518).

SLICE COMPLETE
